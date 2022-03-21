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
use drogue_device::{
    actors::{
        button::{Button, ButtonEvent, ButtonEventHandler},
        led::matrix::LedMatrixActor,
    },
    bsp::boards::nrf52::microbit::*,
    traits::led::LedMatrix as LedMatrixTrait,
    ActorContext, Address, Board,
};

use embassy::util::Forever;
use embassy_nrf::{
    gpio::{AnyPin, Output},
    interrupt,
    peripherals::UARTE0,
    uarte::{self, Uarte},
    Peripherals,
};
use panic_probe as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

#[derive(Default)]
pub struct MyDevice {
    button_a: ActorContext<Button<ButtonA, ButtonAHandler>>,
    button_b: ActorContext<Button<ButtonB, ButtonBHandler>>,
    statistics: ActorContext<Statistics>,
    server: ActorContext<EchoServer<Uarte<'static, UARTE0>>>,
    matrix: ActorContext<AppMatrix>,
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let board = Microbit::new(p);

    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = uarte::Uarte::new(board.uarte0, irq, board.p15, board.p14, config);

    static DEVICE: Forever<MyDevice> = Forever::new();
    let device = DEVICE.put(Default::default());
    let matrix = device
        .matrix
        .mount(spawner, LedMatrixActor::new(board.display, None));
    let statistics = device.statistics.mount(spawner, Statistics::new());
    device
        .server
        .mount(spawner, EchoServer::new(uarte, matrix, statistics));
    device.button_a.mount(
        spawner,
        Button::new(board.btn_a, ButtonAHandler(statistics, matrix)),
    );
    device
        .button_b
        .mount(spawner, Button::new(board.btn_b, ButtonBHandler(matrix)));
}

pub struct ButtonAHandler(Address<Statistics>, Address<AppMatrix>);
impl ButtonEventHandler for ButtonAHandler {
    fn handle(&mut self, event: ButtonEvent) {
        match event {
            ButtonEvent::Pressed => {
                let _ = self.1.increase_brightness();
            }
            ButtonEvent::Released => {
                let _ = self.0.notify(StatisticsCommand::PrintStatistics);
            }
        }
    }
}

pub struct ButtonBHandler(Address<AppMatrix>);
impl ButtonEventHandler for ButtonBHandler {
    fn handle(&mut self, event: ButtonEvent) {
        match event {
            ButtonEvent::Pressed => {
                let _ = self.0.decrease_brightness();
            }
            _ => {}
        }
    }
}
