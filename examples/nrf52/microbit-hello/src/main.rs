#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use defmt_rtt as _;
use drogue_device_kernel::{self as drogue, *};
use embassy_nrf::Peripherals;
use panic_probe as _;

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
        async move { defmt::info!("[{}] started!", self.name) }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: &'m Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            defmt::info!("[{}] hello {}: {}", self.name, message.0, self.counter);
            self.counter += 1;
        }
    }
}

pub struct SayHello<'m>(&'m str);

#[derive(drogue::Device)]
pub struct MyDevice {
    a: ActorState<'static, MyActor>,
    b: ActorState<'static, MyActor>,
}

impl DeviceMounter for MyDevice {
    fn mount(&'static self) {
        self.a.mount(());
        self.b.mount(());
    }
}

#[drogue::configure]
fn configure() -> MyDevice {
    MyDevice {
        a: ActorState::new(MyActor::new("a")),
        b: ActorState::new(MyActor::new("b")),
    }
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    let a_addr = context.device().a.address();
    let b_addr = context.device().b.address();
    loop {
        Timer::after(Duration::from_secs(1)).await;
        a_addr.send(&SayHello("World")).await;
        b_addr.send(&SayHello("You")).await;
    }
}