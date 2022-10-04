#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use heapless::Vec;
use microbit_bsp::*;
use nrf_softdevice::{
    ble::{gatt_server, peripheral, Connection},
    raw, Softdevice,
};
use static_cell::StaticCell;

use defmt_rtt as _;
use panic_probe as _;

#[nrf_softdevice::gatt_server]
pub struct Server {
    bas: BatteryService,
}

#[nrf_softdevice::gatt_service(uuid = "180f")]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let _ = Microbit::new(config());

    // Spawn the underlying softdevice task
    let sd = enable_softdevice("Embassy Microbit");

    // Create a BLE GATT server and make it static
    static SERVER: StaticCell<Server> = StaticCell::new();
    let server = SERVER.init(Server::new(sd).unwrap());

    s.spawn(softdevice_task(sd)).unwrap();

    // Starts the bluetooth advertisement and GATT server
    s.spawn(advertiser_task(s, sd, server, "Embassy Microbit"))
        .unwrap();
}

// Up to 2 connections
#[embassy_executor::task(pool_size = "2")]
pub async fn gatt_server_task(conn: Connection, server: &'static Server) {
    match gatt_server::run(&conn, server, |e| match e {
        ServerEvent::Bas(e) => match e {
            BatteryServiceEvent::BatteryLevelCccdWrite { notifications } => {
                defmt::info!("battery notifications: {}", notifications)
            }
        },
    })
    .await
    {
        Ok(_) => {
            defmt::info!("connection closed");
        }
        Err(e) => {
            defmt::warn!("connection error: {:?}", e);
        }
    }
}

#[embassy_executor::task]
pub async fn advertiser_task(
    spawner: Spawner,
    sd: &'static Softdevice,
    server: &'static Server,
    name: &'static str,
) {
    let mut adv_data: Vec<u8, 31> = Vec::new();
    #[rustfmt::skip]
    adv_data.extend_from_slice(&[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x09, 0x18,
        (1 + name.len() as u8), 0x09]).unwrap();

    adv_data.extend_from_slice(name.as_bytes()).ok().unwrap();

    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data[..],
            scan_data,
        };
        defmt::debug!("advertising");
        let conn = peripheral::advertise_connectable(sd, adv, &config)
            .await
            .unwrap();

        defmt::debug!("connection established");
        if let Err(e) = spawner.spawn(gatt_server_task(conn, server)) {
            defmt::warn!("Error spawning gatt task: {:?}", e);
        }
    }
}

fn enable_softdevice(name: &'static str) -> &'static mut Softdevice {
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
