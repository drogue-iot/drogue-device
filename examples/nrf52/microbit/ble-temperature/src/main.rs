#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use ble::microbit::*;
use drogue_device::{
    actors::led::matrix::{LedMatrixActor, MatrixCommand},
    bsp::boards::nrf52::microbit::*,
    drivers::led::matrix::fonts,
    traits::led::TextDisplay,
    ActorContext, Address, Board, DeviceContext, Package,
};
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Output},
    Peripherals,
};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;
pub struct LedConnectionState(Address<AppMatrix>);

pub struct MyDevice {
    ble_service: MicrobitBleService<LedConnectionState>,
    matrix: ActorContext<AppMatrix>,
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
    let board = Microbit::new(p);

    let device = DEVICE.configure(MyDevice {
        ble_service: MicrobitBleService::new(),
        matrix: ActorContext::new(),
    });

    let mut matrix = device
        .matrix
        .mount(spawner, LedMatrixActor::new(board.display, None));
    matrix.scroll("Hello, Drogue").await.unwrap();
    device
        .ble_service
        .mount(LedConnectionState(matrix), spawner);
}

impl ConnectionStateListener for LedConnectionState {
    type OnConnectedFuture<'m> = impl core::future::Future<Output = ()> + 'm;
    fn on_connected<'m>(&'m self) -> Self::OnConnectedFuture<'m> {
        async move {
            let _ = self.0.notify(MatrixCommand::ApplyFrame(&fonts::CHECK_MARK));
        }
    }

    type OnDisconnectedFuture<'m> = impl core::future::Future<Output = ()> + 'm;
    fn on_disconnected<'m>(&'m self) -> Self::OnDisconnectedFuture<'m> {
        async move {
            self.0.clone().scroll("Disconnected").await.unwrap();
        }
    }
}
