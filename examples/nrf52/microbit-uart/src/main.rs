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
    actors::{
        button::Button,
        led::matrix::{LEDMatrix, MatrixCommand},
        ticker::Ticker,
    },
    ActorContext, DeviceContext,
};

use embassy::time::Duration;
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, NoPin, Output, OutputDrive, Pin, Pull},
    gpiote::PortInput,
    interrupt,
    peripherals::{P0_14, UARTE0},
    uarte::{self, Uarte},
    Peripherals,
};
use panic_probe as _;

type LedMatrix = LEDMatrix<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    button: ActorContext<'static, Button<'static, PortInput<'static, P0_14>, Statistics>>,
    statistics: ActorContext<'static, Statistics>,
    server: ActorContext<'static, EchoServer<'static, Uarte<'static, UARTE0>>>,
    ticker: ActorContext<'static, Ticker<'static, LedMatrix>>,
    matrix: ActorContext<'static, LedMatrix>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let button_port = PortInput::new(Input::new(p.P0_14, Pull::Up));

    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = unsafe { uarte::Uarte::new(p.UARTE0, irq, p.P0_13, p.P0_01, NoPin, NoPin, config) };

    // LED Matrix
    let rows = [
        output_pin(p.P0_21.degrade()),
        output_pin(p.P0_22.degrade()),
        output_pin(p.P0_15.degrade()),
        output_pin(p.P0_24.degrade()),
        output_pin(p.P0_19.degrade()),
    ];

    let cols = [
        output_pin(p.P0_28.degrade()),
        output_pin(p.P0_11.degrade()),
        output_pin(p.P0_31.degrade()),
        output_pin(p.P1_05.degrade()),
        output_pin(p.P0_30.degrade()),
    ];

    let led = LedMatrix::new(rows, cols);

    DEVICE.configure(MyDevice {
        server: ActorContext::new(EchoServer::new(uarte)),
        button: ActorContext::new(Button::new(button_port)),
        statistics: ActorContext::new(Statistics::new()),
        ticker: ActorContext::new(Ticker::new(
            Duration::from_millis(1000 / 200),
            MatrixCommand::Render,
        )),
        matrix: ActorContext::new(led),
    });

    DEVICE.mount(|device| {
        let matrix = device.matrix.mount((), spawner.into());
        let statistics = device.statistics.mount((), spawner.into());
        device.server.mount((matrix, statistics), spawner.into());
        device.button.mount(statistics, spawner.into());
        device.ticker.mount(matrix, spawner.into());
    });
}
