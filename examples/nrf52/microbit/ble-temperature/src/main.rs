#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;

use core::cell::RefCell;
use core::future::Future;
use core::mem;
use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering;
use drogue_device::{ActorContext, Address, DeviceContext};
use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::executor::Spawner;
use embassy::util::Forever;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;

use panic_probe as _;

use fixed::types::I30F2;
use heapless::Vec;
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    peripheral, Connection,
};
use nrf_softdevice::{raw, Softdevice};

mod ble;
use ble::*;

pub struct MyDevice {
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, GattServer>>>,
    gatt: ActorContext<'static, GattServer>,
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

    DEVICE.configure(MyDevice {
        controller: ActorContext::new(controller),
        advertiser: ActorContext::new(BleAdvertiser::new(sd)),
        gatt: ActorContext::new(GattServer::new(sd)),
    });

    DEVICE
        .mount(|device| async move {
            device.controller.mount((), spawner);
            let acceptor = device.gatt.mount((), spawner);
            device.advertiser.mount(acceptor, spawner);
        })
        .await;
}
