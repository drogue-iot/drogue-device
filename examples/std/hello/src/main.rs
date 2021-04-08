#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(generic_associated_types)]
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

    let a1_addr = bind!(device, a1, crate::MyActor, MyActor::new());
    // TODO: Generates SpawnError::Busy
    //let a2_addr = bind!(device, a2, crate::MyActor, MyActor::new());
    loop {
        Timer::after(Duration::from_secs(1)).await;
        a1_addr.send(SayHello("a1")).await;
        //    a2_addr.send(SayHello("a2")).await;
    }
}
