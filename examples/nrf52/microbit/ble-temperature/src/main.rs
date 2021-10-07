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
use heapless::Vec;

use panic_probe as _;

pub struct MyDevice {
    temperature_service: TemperatureService,
    _device_info_service: DeviceInformationService,
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, GattServer>>>,
    gatt: ActorContext<'static, GattServer>,
    monitor: ActorContext<'static, TemperatureMonitor>,
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
    let (controller, sd) = BleController::new("Drogue IoT micro:bit v2.0");

    let mut gatt = GattServer::new(sd);
    let device_info: DeviceInformationService = gatt.register().unwrap();
    device_info
        .model_number_set(Vec::from_slice(b"Drogue IoT micro:bit V2.0").unwrap())
        .unwrap();
    device_info
        .serial_number_set(Vec::from_slice(b"1").unwrap())
        .unwrap();
    device_info
        .manufacturer_name_set(Vec::from_slice(b"BBC").unwrap())
        .unwrap();
    device_info
        .hardware_revision_set(Vec::from_slice(b"1").unwrap())
        .unwrap();

    DEVICE.configure(MyDevice {
        temperature_service: gatt.register().unwrap(),
        _device_info_service: device_info,
        controller: ActorContext::new(controller),
        advertiser: ActorContext::new(BleAdvertiser::new(sd)),
        gatt: ActorContext::new(gatt),
        monitor: ActorContext::new(TemperatureMonitor::new(sd)),
    });

    DEVICE
        .mount(|device| async move {
            device.controller.mount((), spawner);
            let monitor = device.monitor.mount(&device.temperature_service, spawner);
            let acceptor = device
                .gatt
                .mount((&device.temperature_service, monitor), spawner);
            device.advertiser.mount(acceptor, spawner);
        })
        .await;
}
