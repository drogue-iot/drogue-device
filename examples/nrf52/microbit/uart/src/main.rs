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
    drivers::led::matrix::LedMatrix,
    traits::led::LedMatrix as LedMatrixTrait,
    ActorContext, Address, DeviceContext,
};

use embassy_nrf::{
    gpio::{AnyPin, Input, Level, NoPin, Output, OutputDrive, Pin, Pull},
    gpiote::PortInput,
    interrupt,
    peripherals::{P0_14, P0_23, UARTE0},
    uarte::{self, Uarte},
    Peripherals,
};
use panic_probe as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    button_a: ActorContext<'static, Button<PortInput<'static, P0_14>, ButtonA>>,
    button_b: ActorContext<'static, Button<PortInput<'static, P0_23>, ButtonB>>,
    statistics: ActorContext<'static, Statistics>,
    server: ActorContext<'static, EchoServer<'static, Uarte<'static, UARTE0>>>,
    matrix: ActorContext<'static, AppMatrix>,
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

    let a = PortInput::new(Input::new(p.P0_14, Pull::Up));
    let b = PortInput::new(Input::new(p.P0_23, Pull::Up));

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
        button_a: ActorContext::new(Button::new(a)),
        button_b: ActorContext::new(Button::new(b)),
        statistics: ActorContext::new(Statistics::new()),
        matrix: ActorContext::new(LedMatrixActor::new(led, None)),
    });

    DEVICE
        .mount(|device| async move {
            let matrix = device.matrix.mount((), spawner);
            let statistics = device.statistics.mount((), spawner);
            device.server.mount((matrix, statistics), spawner);
            device.button_a.mount(ButtonA(statistics, matrix), spawner);
            device.button_b.mount(ButtonB(matrix), spawner);
        })
        .await;
}

pub struct ButtonA(Address<'static, Statistics>, Address<'static, AppMatrix>);
impl ButtonEventHandler for ButtonA {
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

pub struct ButtonB(Address<'static, AppMatrix>);
impl ButtonEventHandler for ButtonB {
    fn handle(&mut self, event: ButtonEvent) {
        match event {
            ButtonEvent::Pressed => {
                let _ = self.0.decrease_brightness();
            }
            _ => {}
        }
    }
}
