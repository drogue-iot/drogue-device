#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use drogue_device_platform_std::{
    self as drogue, bind, Actor, ActorState, Device, Duration, Forever, Timer,
};

pub struct MyActor {
    counter: u32,
}

impl MyActor {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

pub struct SayHello(&'static str);

#[drogue::actor]
async fn process(state: &mut MyActor, message: SayHello) {
    log::info!("[{}] hello: {}", message.0, state.counter);
    state.counter += 1;
}

#[drogue::main]
async fn main(device: Device) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    // TODO: Generate scaffold
    let a_addr = bind!(device, process, a, crate::MyActor, MyActor::new());
    let b_addr = bind!(device, process, b, crate::MyActor, MyActor::new());
    loop {
        Timer::after(Duration::from_secs(1)).await;
        a_addr.send(SayHello("a")).await;
        b_addr.send(SayHello("b")).await;
    }
}
