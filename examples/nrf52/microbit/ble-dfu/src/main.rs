#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::actors::dfu::FirmwareManager;
use drogue_device::bsp::boards::nrf52::microbit::*;
use drogue_device::drivers::ble::gatt::{dfu::FirmwareGattService, enable_softdevice};
use drogue_device::traits::led::{LedMatrix as _, TextDisplay};
use drogue_device::ActorContext;
use drogue_device::Board;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy::util::Forever;
use embassy_boot_nrf::updater;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;
use nrf_softdevice::ble::gatt_server;
use nrf_softdevice::{raw, Flash, Softdevice};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "nrf-softdevice-defmt-rtt")]
use nrf_softdevice_defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

mod gatt;
pub use gatt::*;

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(s: Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    // Spawn the underlying softdevice task
    let sd = enable_softdevice("DrogueDfu");
    s.spawn(softdevice_task(sd)).unwrap();

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);

    // Watchdog will prevent bootloader from resetting. If your application hangs for more than 5 seconds
    // (depending on bootloader config), it will enter bootloader which may swap the application back.
    s.spawn(watchdog_task()).unwrap();

    // The flash peripheral is special when running with softdevice
    let flash = Flash::take(sd);

    // The updater is the 'application' part of the bootloader that knows where bootloader
    // settings and the firmware update partition is located based on memory.x linker script.
    let updater = updater::new();

    // The DFU actor provides a firmware update interface.
    static DFU: ActorContext<FirmwareManager<Flash>> = ActorContext::new();
    let dfu = DFU.mount(s, FirmwareManager::new(flash, updater));

    // Create a BLE GATT service that is capable of updating firmware
    static GATT: Forever<GattServer> = Forever::new();
    let server = GATT.put(gatt_server::register(sd).unwrap());
    static UPDATER: ActorContext<FirmwareGattService<Flash>, 4> = ActorContext::new();

    // Wires together the GATT service and the DFU actor
    let updater = UPDATER.mount(
        s,
        FirmwareGattService::new(&server.firmware, dfu, version.as_bytes(), 64).unwrap(),
    );

    // Starts the bluetooth advertisement and GATT server
    s.spawn(bluetooth_task(sd, server, updater)).unwrap();

    // LED matrix
    static LED_MATRIX: ActorContext<LedMatrixActor, 3> = ActorContext::new();
    let mut matrix = LED_MATRIX.mount(s, LedMatrixActor::new(board.display, None));

    // Finally, a blinker application.
    loop {
        let _ = matrix.enable();
        let _ = matrix.scroll(version).await;
        let _ = matrix.disable();
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

// Keeps our system alive
#[embassy::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}
