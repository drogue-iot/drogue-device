#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

mod server;
use server::*;

use defmt_rtt as _;
use drogue_device::{
    nrf::{
        gpio::NoPin,
        interrupt,
        peripherals::UARTE0,
        uarte::{self, Uarte},
        Peripherals,
    },
    *,
};
use panic_probe as _;

#[derive(drogue::Device)]
pub struct MyDevice {
    server: ActorState<'static, EchoServer<Uarte<'static, UARTE0>>>,
}

#[drogue::configure]
fn configure() -> MyDevice {
    let p = Peripherals::take().unwrap();

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = unsafe { uarte::Uarte::new(p.UARTE0, irq, p.P0_13, p.P0_01, NoPin, NoPin, config) };
    MyDevice {
        server: ActorState::new(EchoServer::new(uarte)),
    }
}

#[drogue::main]
async fn main(mut context: DeviceContext<MyDevice>) {
    context.device().server.mount(());
    context.start();
}
