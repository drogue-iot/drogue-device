#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use drogue_device::*;

pub struct MyActor {
    name: &'static str,
    counter: u32,
}

impl MyActor {
    pub fn new(name: &'static str) -> Self {
        Self { name, counter: 0 }
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
        mut self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            log::info!("[{}] hello {}: {}", self.name, message.0, self.counter);
            self.counter += 1;
        }
    }
}

pub struct SayHello<'m>(&'m str);

#[derive(drogue::Package)]
pub struct Wrapped {
    b: ActorState<'static, MyActor>,
    c: ActorState<'static, MyActor>,
}

impl Wrapped {
    fn mount(&'static self) -> (Address<'static, MyActor>, Address<'static, MyActor>) {
        (self.b.mount(()), self.c.mount(()))
    }
}

#[derive(Device)]
pub struct MyDevice {
    a: ActorState<'static, MyActor>,
    wrapped: Wrapped,
}

#[drogue::main]
async fn main(mut context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    context.configure(MyDevice {
        a: ActorState::new(MyActor::new("a")),
        wrapped: Wrapped {
            b: ActorState::new(MyActor::new("b")),
            c: ActorState::new(MyActor::new("c")),
        },
    });

    let (a_addr, b_addr, c_addr) = context.mount(|device| {
        let a_addr = device.a.mount(());
        let (b_addr, c_addr) = device.wrapped.mount();
        (a_addr, b_addr, c_addr)
    });

    loop {
        time::Timer::after(time::Duration::from_secs(1)).await;
        a_addr.send(SayHello("World")).await;
        b_addr.send(SayHello("You")).await;
        c_addr.send_ref(&mut SayHello("There")).await;
    }
}
