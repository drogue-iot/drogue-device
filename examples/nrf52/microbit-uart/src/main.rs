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
mod statistics;
use server::*;
use statistics::*;

use defmt_rtt as _;
use drogue_device::{
    actors::button::Button,
    nrf::{
        gpio::{Input, NoPin, Pull},
        gpiote::{self, PortInput},
        interrupt,
        peripherals::{P0_14, UARTE0},
        uarte::{self, Uarte},
        Peripherals,
    },
    *,
};
use panic_probe as _;

#[derive(drogue::Device)]
pub struct MyDevice {
    button: ActorState<'static, Button<PortInput<'static, P0_14>, StatisticsCommand, Statistics>>,
    statistics: ActorState<'static, Statistics>,
    server: ActorState<'static, EchoServer<'static, Uarte<'static, UARTE0>>>,
}

#[drogue::configure]
fn configure() -> MyDevice {
    let p = Peripherals::take().unwrap();

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let g = gpiote::initialize(p.GPIOTE, interrupt::take!(GPIOTE));
    let button_port = PortInput::new(g, Input::new(p.P0_14, Pull::Up));

    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = unsafe { uarte::Uarte::new(p.UARTE0, irq, p.P0_13, p.P0_01, NoPin, NoPin, config) };
    MyDevice {
        server: ActorState::new(EchoServer::new(uarte)),
        button: ActorState::new(Button::new(button_port)),
        statistics: ActorState::new(Statistics::new()),
    }
}

#[drogue::main]
async fn main(mut context: DeviceContext<MyDevice>) {
    let statistics = context.device().statistics.mount(());
    context.device().server.mount(statistics);
    context.device().button.mount(statistics);
    context.start();
}
