#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use drogue_device::drivers::ble::gatt::{
    device_info::{DeviceInformationService, DeviceInformationServiceEvent},
    environment::*,
};
use drogue_device::shared::Shared;
use drogue_device::traits::led::ToFrame;
use drogue_device::Board;
use drogue_device::{bsp::boards::nrf52::microbit::Microbit, domain::led::matrix::Brightness};
use drogue_device::{
    drivers::ble::gatt::dfu::{FirmwareGattService, FirmwareService, FirmwareServiceEvent},
    firmware::{FirmwareManager, SharedFirmwareManager},
};
use embassy_util::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_util::channel::mpmc::{Channel, DynamicReceiver, DynamicSender};
use embassy_executor::executor::Spawner;
use embassy_executor::time::Delay;
use embassy_executor::time::Ticker;
use embassy_executor::time::{Duration, Timer};
use embassy_util::Forever;
use embassy_util::{select, Either};
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    interrupt,
    peripherals::{TIMER0, UARTE0},
    uarte, Peripherals,
};
use futures::StreamExt;
use heapless::Vec;
use nrf_softdevice::ble::gatt_server;
use nrf_softdevice::ble::peripheral;
use nrf_softdevice::{ble::Connection, raw, temperature_celsius, Flash, Softdevice};

#[cfg(feature = "dfu")]
use embassy_boot_nrf::FirmwareUpdater;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy_executor::main(config = "config()")]
async fn main(s: Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    // Spawn the underlying softdevice task
    let sd = enable_softdevice("Drogue Low Energy");
    s.spawn(softdevice_task(sd)).unwrap();

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);

    // Create a BLE GATT server and make it static
    static GATT: Forever<GattServer> = Forever::new();
    let server = GATT.put(gatt_server::register(sd).unwrap());
    server
        .device_info
        .initialize(b"Drogue Low Energy", b"1.0", b"Red Hat", b"1.0")
        .unwrap();
    server
        .env
        .descriptor_set(
            MeasurementDescriptor {
                flags: 0,
                sampling_fn: SamplingFunction::ArithmeticMean,
                measurement_period: Period::Unknown,
                update_interval: Interval::Value(5),
                application: MeasurementApp::Air,
                uncertainty: Uncertainty::Unknown,
            }
            .to_vec(),
        )
        .unwrap();
    server
        .env
        .trigger_set(TriggerSetting::FixedInterval(5).to_vec())
        .unwrap();

    // Firmware update service event channel and task
    static EVENTS: Channel<ThreadModeRawMutex, FirmwareServiceEvent, 10> = Channel::new();
    #[cfg(feature = "dfu")]
    {
        // The updater is the 'application' part of the bootloader that knows where bootloader
        // settings and the firmware update partition is located based on memory.x linker script.
        static DFU: Shared<FirmwareManager<Flash, 4096, 64>> = Shared::new();
        let dfu = DFU.initialize(FirmwareManager::new(
            Flash::take(sd),
            FirmwareUpdater::default(),
            version.as_bytes(),
        ));
        let updater =
            FirmwareGattService::new(&server.firmware, dfu.clone(), version.as_bytes(), 64)
                .unwrap();
        s.spawn(updater_task(updater, EVENTS.receiver().into()))
            .unwrap();

        // Watchdog will prevent bootloader from resetting. If your application hangs for more than 5 seconds
        // (depending on bootloader config), it will enter bootloader which may swap the application back.
        s.spawn(watchdog_task()).unwrap();
    }

    // Starts the bluetooth advertisement and GATT server
    s.spawn(advertiser_task(
        s,
        sd,
        server,
        EVENTS.sender().into(),
        "Drogue Low Energy",
    ))
    .unwrap();

    // Finally, a blinker application.
    let mut display = board.display;
    display.set_brightness(Brightness::MAX);
    loop {
        let _ = display
            .display('A'.to_frame(), Duration::from_secs(1))
            .await;
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[cfg(feature = "dfu")]
#[nrf_softdevice::gatt_server]
pub struct GattServer {
    pub firmware: FirmwareService,
    pub env: EnvironmentSensingService,
    pub device_info: DeviceInformationService,
}

#[cfg(not(feature = "dfu"))]
#[nrf_softdevice::gatt_server]
pub struct GattServer {
    pub env: EnvironmentSensingService,
    pub device_info: DeviceInformationService,
}

#[embassy_executor::task]
pub async fn updater_task(
    mut dfu: FirmwareGattService<'static, SharedFirmwareManager<'static, Flash, 4096, 64>>,
    events: DynamicReceiver<'static, FirmwareServiceEvent>,
) {
    loop {
        let event = events.recv().await;
        if let Err(e) = dfu.handle(&event).await {
            defmt::warn!("Error applying firmware event: {:?}", e);
        }
    }
}

#[embassy_executor::task(pool_size = "4")]
pub async fn gatt_server_task(
    sd: &'static Softdevice,
    conn: Connection,
    server: &'static GattServer,
    events: DynamicSender<'static, FirmwareServiceEvent>,
) {
    let mut notify = false;
    let mut ticker = Ticker::every(Duration::from_secs(5));
    let env_service = &server.env;
    loop {
        let mut interval = None;
        let next = ticker.next();
        match select(
            gatt_server::run(&conn, server, |e| match e {
                GattServerEvent::Env(e) => match e {
                    EnvironmentSensingServiceEvent::TemperatureCccdWrite { notifications } => {
                        notify = notifications;
                    }
                    EnvironmentSensingServiceEvent::PeriodWrite(period) => {
                        defmt::info!("Setting interval to {} seconds", period);
                        interval.replace(Duration::from_secs(period as u64));
                    }
                },
                #[cfg(feature = "dfu")]
                GattServerEvent::Firmware(e) => {
                    let _ = events.try_send(e);
                }
                _ => {}
            }),
            next,
        )
        .await
        {
            Either::First(res) => {
                if let Err(e) = res {
                    defmt::warn!("gatt_server run exited with error: {:?}", e);
                    return;
                }
            }
            Either::Second(_) => {
                let value: i8 = temperature_celsius(sd).unwrap().to_num();
                defmt::info!("Measured temperature: {}℃", value);
                let value = value as i16 * 10;

                env_service.temperature_set(value).unwrap();
                if notify {
                    defmt::trace!("Notifying");
                    env_service.temperature_notify(&conn, value).unwrap();
                }
            }
        }

        if let Some(interval) = interval.take() {
            ticker = Ticker::every(interval);
        }
    }
}

#[embassy_executor::task]
pub async fn advertiser_task(
    spawner: Spawner,
    sd: &'static Softdevice,
    server: &'static GattServer,
    events: DynamicSender<'static, FirmwareServiceEvent>,
    name: &'static str,
) {
    let mut adv_data: Vec<u8, 31> = Vec::new();
    #[rustfmt::skip]
    adv_data.extend_from_slice(&[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x1A, 0x18,
        (1 + name.len() as u8), 0x09]).unwrap();

    adv_data.extend_from_slice(name.as_bytes()).ok().unwrap();

    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x0A, 0x18,
    ];

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data[..],
            scan_data,
        };
        defmt::debug!("Advertising");
        let conn = peripheral::advertise_connectable(sd, adv, &config)
            .await
            .unwrap();

        defmt::debug!("connection established");
        if let Err(e) = spawner.spawn(gatt_server_task(sd, conn, server, events.clone())) {
            defmt::warn!("Error spawning gatt task: {:?}", e);
        }
    }
}

fn enable_softdevice(name: &'static str) -> &'static Softdevice {
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
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 128 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: name.as_ptr() as *const u8 as _,
            current_len: name.len() as u16,
            max_len: name.len() as u16,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };
    Softdevice::enable(&config)
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

// Keeps our system alive
#[embassy_executor::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}
