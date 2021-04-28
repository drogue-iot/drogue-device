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
    nrf::{
        gpio::{AnyPin, Input, Level, NoPin, Output, OutputDrive, Pin, Pull},
        gpiote::{self, PortInput},
        interrupt,
        peripherals::{P0_14, UARTE0},
        uarte::{self, Uarte},
        Peripherals,
    },
    time::Duration,
    *,
};
use heapless::{consts, Vec};
use panic_probe as _;

type LedMatrix = LEDMatrix<Output<'static, AnyPin>, consts::U5, consts::U5>;

#[derive(Device)]
pub struct MyDevice {
    button: ActorContext<'static, Button<'static, PortInput<'static, P0_14>, Statistics>>,
    statistics: ActorContext<'static, Statistics>,
    server: ActorContext<'static, EchoServer<'static, Uarte<'static, UARTE0>>>,
    ticker: ActorContext<'static, Ticker<'static, LedMatrix>>,
    matrix: ActorContext<'static, LedMatrix>,
}

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    let p = Peripherals::take().unwrap();

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let g = gpiote::initialize(p.GPIOTE, interrupt::take!(GPIOTE));
    let button_port = PortInput::new(g, Input::new(p.P0_14, Pull::Up));

    let irq = interrupt::take!(UARTE0_UART0);
    let uarte = unsafe { uarte::Uarte::new(p.UARTE0, irq, p.P0_13, p.P0_01, NoPin, NoPin, config) };

    // LED Matrix
    let mut rows = Vec::<_, consts::U5>::new();
    rows.push(output_pin(p.P0_21.degrade())).ok().unwrap();
    rows.push(output_pin(p.P0_22.degrade())).ok().unwrap();
    rows.push(output_pin(p.P0_15.degrade())).ok().unwrap();
    rows.push(output_pin(p.P0_24.degrade())).ok().unwrap();
    rows.push(output_pin(p.P0_19.degrade())).ok().unwrap();

    let mut cols = Vec::<_, consts::U5>::new();
    cols.push(output_pin(p.P0_28.degrade())).ok().unwrap();
    cols.push(output_pin(p.P0_11.degrade())).ok().unwrap();
    cols.push(output_pin(p.P0_31.degrade())).ok().unwrap();
    cols.push(output_pin(p.P1_05.degrade())).ok().unwrap();
    cols.push(output_pin(p.P0_30.degrade())).ok().unwrap();

    let led = LedMatrix::new(rows, cols);

    context.configure(MyDevice {
        server: ActorContext::new(EchoServer::new(uarte)),
        button: ActorContext::new(Button::new(button_port)),
        statistics: ActorContext::new(Statistics::new()),
        ticker: ActorContext::new(Ticker::new(
            Duration::from_millis(1000 / 200),
            MatrixCommand::Render,
        )),
        matrix: ActorContext::new(led),
    });

    context.mount(|device| {
        let matrix = device.matrix.mount(());
        let statistics = device.statistics.mount(());
        device.server.mount((matrix, statistics));
        device.button.mount(statistics);
        device.ticker.mount(matrix);
    });
}
