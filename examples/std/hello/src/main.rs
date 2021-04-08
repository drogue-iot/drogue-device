#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use drogue_device_platform_std::{
    self as drogue, bind, Actor, ActorState, Device, DeviceContext, Duration, Timer,
};

pub struct MyActor {
    counter: u32,
}

impl MyActor {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl Actor for MyActor {
    type Message = SayHello;
    type ProcessFuture<'a> = impl Future<Output = ()> + 'a;

    fn process<'a>(mut self: Pin<&'a mut Self>, message: Self::Message) -> Self::ProcessFuture<'a> {
        async move {
            log::info!("[{}] hello: {}", message.0, self.counter);
            self.counter += 1;
        }
    }
}

pub struct SayHello(&'static str);

#[derive(Device)]
pub struct MyDevice {
    a: ActorState<'static, MyActor>,
    b: ActorState<'static, MyActor>,
}

/*
impl Device for MyDevice {
    fn mount(&'static self) {
        self.a.mount();
        self.b.mount();
    }
}*/

fn configure() -> MyDevice {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    MyDevice {
        a: ActorState::new(MyActor::new()),
        b: ActorState::new(MyActor::new()),
    }
}

#[drogue::main(configure)]
async fn main(context: DeviceContext<MyDevice>) {
    let a_addr = context.device().a.address();
    let b_addr = context.device().b.address();
    loop {
        Timer::after(Duration::from_secs(1)).await;
        a_addr.send(SayHello("a")).await;
        b_addr.send(SayHello("b")).await;
    }
}
