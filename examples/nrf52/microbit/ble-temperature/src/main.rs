#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;

use core::cell::RefCell;
use core::mem;
use core::sync::atomic::AtomicU16;
use core::sync::atomic::Ordering;
use embassy::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy::executor::Spawner;
use embassy::util::Forever;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;

use panic_probe as _;

use heapless::Vec;
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    peripheral, Connection,
};
use nrf_softdevice::{raw, temperature_celsius, Softdevice};

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

#[nrf_softdevice::gatt_server(uuid = "e95d6100-251d-470a-a062-fa1922dfa9a8")]
struct TemperatureService {
    #[characteristic(uuid = "e95d9250-251d-470a-a062-fa1922dfa9a8", read, notify)]
    temperature: i8,
    #[characteristic(uuid = "e95d1b25-251d-470a-a062-fa1922dfa9a8", read, write)]
    period: u16,
}

#[nrf_softdevice::gatt_server(uuid = "0000180a-0000-1000-8000-00805f9b34fb")]
struct DeviceInformationService {
    #[characteristic(uuid = "00002a24-0000-1000-8000-00805f9b34fb", read)]
    model_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a25-0000-1000-8000-00805f9b34fb", read)]
    serial_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a27-0000-1000-8000-00805f9b34fb", read)]
    hardware_revision: Vec<u8, 4>,
    #[characteristic(uuid = "00002a29-0000-1000-8000-00805f9b34fb", read)]
    manufacturer_name: Vec<u8, 32>,
}

static INTERVAL: AtomicU16 = AtomicU16::new(10_000);

static CONNECTIONS: CriticalSectionMutex<RefCell<Vec<Connection, 2>>> =
    CriticalSectionMutex::new(RefCell::new(Vec::new()));

#[embassy::task]
async fn temperature_monitor(sd: &'static Softdevice, service: &'static TemperatureService) {
    loop {
        let interval = INTERVAL.load(Ordering::SeqCst);
        embassy::time::Timer::after(embassy::time::Duration::from_millis(interval.into())).await;
        let value: i8 = temperature_celsius(sd).unwrap().to_num();
        defmt::trace!("Measured temperature: {}℃", value);

        service.temperature_set(value).unwrap();

        CONNECTIONS.lock(|c| {
            let c = c.borrow();
            for c in c.iter() {
                service.temperature_notify(&c, value).unwrap();
            }
        });
    }
}

#[embassy::task]
async fn bluetooth_task(
    sd: &'static Softdevice,
    temperature: &'static TemperatureService,
    _device_info: &'static DeviceInformationService,
) {
    #[rustfmt::skip]
    let adv_data = &[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x09, 0x18,
        0x12, 0x09, b'D', b'r', b'o', b'g', b'u', b'e', b' ', b'L', b'o', b'w', b' ', b'E',b'n', b'e', b'r', b'g', b'y',
    ];
    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];
    defmt::info!("advertising started!");

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data,
            scan_data,
        };
        let conn = peripheral::advertise_connectable(sd, adv, &config)
            .await
            .unwrap();

        defmt::info!("advertising done!");

        // Run the GATT server on the connection. This returns when the connection gets disconnected.
        let res = gatt_server::run(&conn, |e| {
            temperature.on_event(&e, |e| match e {
                TemperatureServiceEvent::TemperatureNotificationsEnabled => {
                    CONNECTIONS
                        .lock(|c| c.borrow_mut().push(conn.clone()))
                        .ok()
                        .unwrap();
                    defmt::info!("notifications enabled!");
                }
                TemperatureServiceEvent::TemperatureNotificationsDisabled => {
                    CONNECTIONS.lock(|c| {
                        let mut c = c.borrow_mut();
                        let mut v_new = Vec::new();
                        for c in c.iter() {
                            if c.handle() != conn.handle() {
                                v_new.push(c.clone()).ok().unwrap();
                            }
                        }
                        *c = v_new;
                    });
                    defmt::info!("notifications disabled!");
                }
                TemperatureServiceEvent::PeriodWrite(period) => {
                    defmt::info!("adjust period!");
                    INTERVAL.store(period, Ordering::SeqCst);
                }
            });
        })
        .await;

        if let Err(e) = res {
            defmt::info!("gatt_server exited with error: {:?}", e);
        }
    }
}

fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, _p: Peripherals) {
    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 4,
            rc_temp_ctiv: 2,
            accuracy: 7,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"Drogue Low Energy" as *const u8 as _,
            current_len: 17,
            max_len: 17,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };
    let sd = Softdevice::enable(&config);

    let device_service: DeviceInformationService = gatt_server::register(sd).unwrap();
    device_service
        .model_number_set(Vec::from_slice("Drogue IoT micro:bit v2.0".as_bytes()).unwrap())
        .unwrap();
    device_service
        .serial_number_set(Vec::from_slice("1".as_bytes()).unwrap())
        .unwrap();
    device_service
        .manufacturer_name_set(Vec::from_slice("BBC".as_bytes()).unwrap())
        .unwrap();
    device_service
        .hardware_revision_set(Vec::from_slice("1".as_bytes()).unwrap())
        .unwrap();

    static TEMPERATURE: Forever<TemperatureService> = Forever::new();
    let temperature = TEMPERATURE.put(gatt_server::register(sd).unwrap());

    static DEVICE_INFO: Forever<DeviceInformationService> = Forever::new();
    let device_info = DEVICE_INFO.put(device_service);

    defmt::unwrap!(spawner.spawn(softdevice_task(sd)));
    defmt::unwrap!(spawner.spawn(bluetooth_task(sd, temperature, device_info)));
    defmt::unwrap!(spawner.spawn(temperature_monitor(sd, temperature)));
}
