#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]

use drogue_device_platform_std::{self as drogue,
Device, Actor, Forever, Duration, Timer, ActorState};

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

    // TODO: Generate scaffold
    static A1: Forever<ActorState<'static, MyActor>> = Forever::new();

    #[drogue::main]
    async fn main(device: Device) {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .format_timestamp_nanos()
            .init();

        // TODO: Generate scaffold
        let a = A1.put(ActorState::new(MyActor::new()));
        let a_addr = a.mount();
        device.start(__drogue_process_trampoline(a));
        loop {
            Timer::after(Duration::from_secs(1)).await;
            a_addr.send(SayHello).await;
        }
    }
