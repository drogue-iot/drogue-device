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
    counter: Option<&'static AtomicU32>,
}

impl MyActor {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            counter: None,
        }
    }
}

impl Actor for MyActor {
    type Configuration = &'static AtomicU32;
    type Message<'a> = SayHello<'a>;
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.counter.replace(config);
    }

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move { log::info!("[{}] started!", self.name) }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let count = self.counter.unwrap().fetch_add(1, Ordering::SeqCst);
            log::info!("[{}] hello {}: {}", self.name, message.0, count);
        }
    }
}

pub struct SayHello<'m>(&'m str);

#[derive(Device)]
pub struct MyDevice {
    counter: AtomicU32,
    a: ActorContext<'static, MyActor>,
    b: ActorContext<'static, MyActor>,
    p: PackageContext<MyPack>,
}

// A package is a way to wrap a package of actors and shared state together
// the actor in this package will use a different state than the others.
#[derive(Package)]
pub struct MyPack {
    counter: AtomicU32,
    c: ActorContext<'static, MyActor>,
}

#[drogue::main]
async fn main(mut context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    context.configure(MyDevice {
        counter: AtomicU32::new(0),
        a: ActorContext::new(MyActor::new("a")),
        b: ActorContext::new(MyActor::new("b")),
        p: PackageContext::new(MyPack {
            counter: AtomicU32::new(0),
            c: ActorContext::new(MyActor::new("c")),
        }),
    });

    let (a_addr, b_addr, c_addr) = context.mount(|device| {
        let a_addr = device.a.mount(&device.counter);
        let b_addr = device.b.mount(&device.counter);
        let c_addr = device.p.mount(|p| p.c.mount(&p.counter));
        (a_addr, b_addr, c_addr)
    });

    loop {
        time::Timer::after(time::Duration::from_secs(1)).await;
        // Send that completes when message is enqueued
        a_addr.notify(SayHello("World")).await;
        // Send that waits until message is processed
        b_addr.process(&mut SayHello("You")).await;

        // Actor uses a different counter
        c_addr.notify(SayHello("There")).await;
    }
}
