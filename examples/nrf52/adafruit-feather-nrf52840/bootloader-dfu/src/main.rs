#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::actors::dfu::FirmwareManager;
//use drogue_device::actors::flash::{SharedFlash, SharedFlashHandle};
//use drogue_device::actors::usb::dfu::SerialUpdater;
use drogue_device::ActorContext;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy::util::Forever;
use embassy_boot_nrf::updater;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
//use embassy_nrf::peripherals::USBD;
//use embassy_nrf::usb::UsbBus;
use embassy_nrf::{
    gpio::{AnyPin, Level, Output, OutputDrive, Pin},
    Peripherals,
};
use nrf_softdevice::ble::gatt_server;
use nrf_softdevice::{raw, Flash, Softdevice};
//use nrf_usbd::Usbd;
//use usb_device::bus::UsbBusAllocator;

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
    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 4,
            rc_temp_ctiv: 2,
            accuracy: 7,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 2,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"DrogueDfu" as *const u8 as _,
            current_len: 9,
            max_len: 9,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };
    let sd = Softdevice::enable(&config);
    s.spawn(softdevice_task(sd)).unwrap();

    // Watchdog will prevent bootloader from resetting. If your application hangs, it will enter bootloader which may swap the application back.
    s.spawn(watchdog_task()).unwrap();

    let flash = Flash::take(sd);

    // The SharedFlash actor allows multiple components and tasks to access the flash.
    /*
    static FLASH: ActorContext<SharedFlash<Flash>> = ActorContext::new();
    let flash = FLASH.mount(s, SharedFlash::new(flash));
    */
    // The updater knows where bootloader settings and the firmware update partition is located based on memory.x
    let updater = updater::new();

    /// The DFU actor allows you to write to flash
    //static DFU: ActorContext<FirmwareManager<SharedFlashHandle<Flash>>> = ActorContext::new();
    static DFU: ActorContext<FirmwareManager<Flash>> = ActorContext::new();
    let dfu = DFU.mount(s, FirmwareManager::new(flash, updater));

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);

    defmt::info!("Running firmware version {}", version);

    // The GATT server provides a custom FirmwareUpdateService GATT service
    static GATT: Forever<GattServer> = Forever::new();
    let server = GATT.put(gatt_server::register(sd).unwrap());
    server
        .firmware
        .version_set(heapless::Vec::from_slice(version.as_bytes()).unwrap())
        .unwrap();
    static UPDATER: ActorContext<FirmwareUpdater, 4> = ActorContext::new();

    // Wires together the GATT service and the DFU actor
    let updater = UPDATER.mount(s, FirmwareUpdater::new(&server.firmware, dfu));

    // Wires together USB and DFU actor
    /*
    static USB: Forever<UsbBusAllocator<Usbd<UsbBus<'static, USBD>>>> = Forever::new();
    let bus = USB.put(UsbBus::new(p.USBD));
    static mut TX: [u8; 1024] = [0; 1024];
    static mut RX: [u8; 1024] = [0; 1024];
    static SERIAL: ActorContext<SerialUpdater<'static, SharedFlashHandle<Flash>>> =
        ActorContext::new();
    SERIAL.mount(s, unsafe {
        SerialUpdater::new(bus, &mut TX, &mut RX, dfu, version)
    });
    */

    // Starts the bluetooth advertisement and GATT server
    s.spawn(bluetooth_task(sd, server, updater)).unwrap();

    // The blinker
    let led = Output::new(p.P1_10.degrade(), Level::High, OutputDrive::Standard);
    s.spawn(blinker(led)).unwrap();
}

const BLINK_INTERVAL: Duration = Duration::from_millis(300);

#[embassy::task]
async fn blinker(mut led: Output<'static, AnyPin>) {
    loop {
        Timer::after(BLINK_INTERVAL).await;
        led.set_low();
        Timer::after(BLINK_INTERVAL).await;
        led.set_high();
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
