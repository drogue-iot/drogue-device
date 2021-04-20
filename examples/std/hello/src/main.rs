#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU32, Ordering};
use drogue_device::*;

pub struct MyActor {
    name: &'static str,
    counter: &'static AtomicU32,
}

impl MyActor {
    pub fn new(name: &'static str, counter: &'static AtomicU32) -> Self {
        Self { name, counter }
    }
}

impl Actor for MyActor {
    type Configuration = ();
    type Message<'a> = SayHello<'a>;
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move { log::info!("[{}] started!", self.name) }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let count = self.counter.fetch_add(1, Ordering::SeqCst);
            log::info!("[{}] hello {}: {}", self.name, message.0, count);
        }
    }
}

pub struct SayHello<'m>(&'m str);

#[derive(Device)]
pub struct MyDevice {
    a: ActorState<'static, MyActor>,
    b: ActorState<'static, MyActor>,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[drogue::main]
async fn main(mut context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    context.configure(MyDevice {
        a: ActorState::new(MyActor::new("a", &COUNTER)),
        b: ActorState::new(MyActor::new("b", &COUNTER)),
    });

    let (a_addr, b_addr) = context.mount(|device| {
        let a_addr = device.a.mount(());
        let b_addr = device.b.mount(());
        (a_addr, b_addr)
    });

    loop {
        time::Timer::after(time::Duration::from_secs(1)).await;
        // Notify waits until message is enqueued
        a_addr.notify(SayHello("World")).await;
        // Send waits until message is processed
        b_addr.send(&mut SayHello("You")).await;
    }
}
