#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use ble::microbit::*;
use drogue_device::{
    actors::led::matrix::{AnimationEffect, LedMatrixActor, MatrixCommand},
    drivers::led::matrix::{fonts, LedMatrix},
    ActorContext, Address, DeviceContext, Package,
};
use embassy::executor::Spawner;
use embassy::time::Duration;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Level, Output, OutputDrive, Pin},
    Peripherals,
};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5, 128>;
pub struct LedConnectionState(Address<'static, AppMatrix>);

pub struct MyDevice {
    ble_service: MicrobitBleService<LedConnectionState>,
    matrix: ActorContext<'static, AppMatrix>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
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
        ble_service: MicrobitBleService::new(),
        matrix: ActorContext::new(LedMatrixActor::new(Duration::from_millis(1000 / 200), led)),
    });

    DEVICE
        .mount(|device| async move {
            let matrix = device.matrix.mount((), spawner);

            device
                .ble_service
                .mount(LedConnectionState(matrix), spawner);
        })
        .await;
}

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

impl ConnectionStateListener for LedConnectionState {
    fn on_connected(&self) {
        self.0
            .notify(MatrixCommand::ApplyFrame(&fonts::CHECK_MARK))
            .unwrap();
    }
    fn on_disconnected(&self) {
        self.0
            .notify(MatrixCommand::ApplyText(
                "Disconnected",
                AnimationEffect::Slide,
                Duration::from_secs(6),
            ))
            .unwrap()
    }
}
