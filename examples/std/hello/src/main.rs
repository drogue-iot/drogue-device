#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use drogue_device_platform_std::{
    self as drogue, bind, Actor, ActorState, Device, Duration, Timer,
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

#[drogue::main]
async fn main(device: Device) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    // TODO: Generate scaffold
    let a_addr = bind!(device, a, crate::MyActor, MyActor::new());
    let b_addr = bind!(device, b, crate::MyActor, MyActor::new());
    loop {
        Timer::after(Duration::from_secs(1)).await;
        a_addr.send(SayHello("a")).await;
        b_addr.send(SayHello("b")).await;
    }
}
