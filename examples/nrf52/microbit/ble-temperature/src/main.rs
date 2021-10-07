#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;

use ble::*;
use drogue_device::{ActorContext, Address, DeviceContext};
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;

use panic_probe as _;

pub struct MyDevice {
    service: TemperatureService,
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, GattServer>>>,
    gatt: ActorContext<'static, GattServer>,
    monitor: ActorContext<'static, TemperatureMonitor>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, _p: Peripherals) {
    let (controller, sd) = BleController::new("Drogue IoT micro:bit v2.0");

    let mut gatt = GattServer::new(sd);
    DEVICE.configure(MyDevice {
        service: gatt.register().unwrap(),
        controller: ActorContext::new(controller),
        advertiser: ActorContext::new(BleAdvertiser::new(sd)),
        gatt: ActorContext::new(gatt),
        monitor: ActorContext::new(TemperatureMonitor::new(sd)),
    });

    DEVICE
        .mount(|device| async move {
            device.controller.mount((), spawner);
            let monitor = device.monitor.mount(&device.service, spawner);
            let acceptor = device.gatt.mount((&device.service, monitor), spawner);
            device.advertiser.mount(acceptor, spawner);
        })
        .await;
}
