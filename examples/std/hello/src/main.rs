#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(generic_associated_types)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use drogue_device_platform_std::{
    self as drogue, bind, Actor, ActorState, Device, Duration, Forever, Timer,
};

struct MyActor {
    counter: u32,
}

impl MyActor {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

struct SayHello;

#[drogue::actor]
async fn process(state: &mut MyActor, _: SayHello) {
    log::info!("Hello: {}", state.counter);
    state.counter += 1;
}

#[drogue::main]
async fn main(device: Device) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    // TODO: Generate scaffold
    let addr = bind!(device, MyActor = MyActor::new());
    loop {
        Timer::after(Duration::from_secs(1)).await;
        addr.send(SayHello).await;
    }
}
