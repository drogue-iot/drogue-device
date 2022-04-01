#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod server;
mod statistics;

use server::*;
use statistics::*;

use defmt_rtt as _;
use drogue_device::{actors::button::Button, bsp::boards::nrf52::microbit::*, ActorContext, Board};

use embassy::util::Forever;
use embassy_nrf::{
    interrupt,
    peripherals::UARTE0,
    uarte::{self, Uarte},
    Peripherals,
};
use panic_probe as _;

pub type AppMatrix = LedMatrix;

#[derive(Default)]
pub struct MyDevice {
    button_a: ActorContext<Button<PinButtonA, StatisticsCommand>>,
    statistics: ActorContext<Statistics>,
    server: ActorContext<EchoServer<Uarte<'static, UARTE0>>>,
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;
    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = uarte::Uarte::new(board.uarte0, irq, board.p15, board.p14, config);

    static DEVICE: Forever<MyDevice> = Forever::new();
    let device = DEVICE.put(Default::default());
    let statistics = device.statistics.mount(spawner, Statistics::new());
    device
        .server
        .mount(spawner, EchoServer::new(uarte, board.display, statistics));
    device
        .button_a
        .mount(spawner, Button::new(board.btn_a, statistics));
}
