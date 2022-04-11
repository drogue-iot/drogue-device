use drogue_device::drivers::ble::gatt::dfu::{FirmwareService, FirmwareServiceEvent};
use drogue_device::Address;
use nrf_softdevice::{
    ble::{gatt_server, peripheral},
    raw, Softdevice,
};

#[nrf_softdevice::gatt_server]
pub struct GattServer {
    pub firmware: FirmwareService,
}

// THe task running the BLE advertisement and discovery
#[embassy::task]
pub async fn bluetooth_task(
    sd: &'static Softdevice,
    server: &'static GattServer,
    dfu: Address<FirmwareServiceEvent>,
) {
    #[rustfmt::skip]
    let adv_data = &[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x60, 0x18,
        0x0a, 0x09, b'D', b'r', b'o', b'g', b'u', b'e', b'D', b'f', b'u',
    ];

    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data,
            scan_data,
        };
        defmt::info!("Advertising");
        let conn = peripheral::advertise_connectable(sd, adv, &config)
            .await
            .unwrap();
        defmt::info!("connection established");
        let res = gatt_server::run(&conn, server, |e| match e {
            GattServerEvent::Firmware(e) => {
                let _ = dfu.notify(e);
            }
        })
        .await;

        if let Err(e) = res {
            defmt::warn!("gatt_server run exited with error: {:?}", e);
        }
    }
}
