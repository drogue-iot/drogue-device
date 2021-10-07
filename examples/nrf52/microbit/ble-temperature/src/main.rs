#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;

use ble::microbit::*;
use drogue_device::{DeviceContext, Package};
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;

use panic_probe as _;

pub struct MyDevice {
    ble_service: MicrobitBleService,
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
async fn main(spawner: Spawner, _p: Peripherals) {
    DEVICE.configure(MyDevice {
        ble_service: MicrobitBleService::new(),
    });

    DEVICE
        .mount(|device| async move {
            device.ble_service.mount((), spawner);
        })
        .await;
}
