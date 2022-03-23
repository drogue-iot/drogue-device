use core::future::Future;
use drogue_device::actors::dfu::{DfuCommand, FirmwareManager};
use drogue_device::{Actor, Address, Inbox};
use heapless::Vec;
use nrf_softdevice::{
    ble::{gatt_server, peripheral},
    raw, Flash, Softdevice,
};

#[nrf_softdevice::gatt_server]
pub struct GattServer {
    pub firmware: FirmwareUpdateService,
}

// The FirmwareUpdate proprietary GATT service
#[nrf_softdevice::gatt_service(uuid = "1861")]
pub struct FirmwareUpdateService {
    #[characteristic(uuid = "1234", write)]
    firmware: Vec<u8, 128>,

    #[characteristic(uuid = "1235", read)]
    offset: u32,

    #[characteristic(uuid = "1236", write)]
    control: u8,

    #[characteristic(uuid = "1237", read)]
    pub version: Vec<u8, 16>,
}

// THe task running the BLE advertisement and discovery
#[embassy::task]
pub async fn bluetooth_task(
    sd: &'static Softdevice,
    server: &'static GattServer,
    dfu: Address<GattUpdater>,
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

pub struct GattUpdater {
    service: &'static FirmwareUpdateService,
    dfu: Address<FirmwareManager<Flash>>,
}

impl GattUpdater {
    pub fn new(
        service: &'static FirmwareUpdateService,
        dfu: Address<FirmwareManager<Flash>>,
    ) -> Self {
        Self { service, dfu }
    }
}

impl Actor for GattUpdater {
    type Message<'m> = FirmwareUpdateServiceEvent;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    match m.message() {
                        FirmwareUpdateServiceEvent::ControlWrite(value) => {
                            defmt::info!("Processing control message {}", value);
                            if *value == 1 {
                                self.service.offset_set(0).ok();
                                self.dfu.request(DfuCommand::Start).unwrap().await.unwrap();
                            } else if *value == 2 {
                                self.dfu.notify(DfuCommand::Finish).unwrap();
                            } else if *value == 3 {
                                self.dfu.notify(DfuCommand::Booted).unwrap();
                            }
                        }
                        FirmwareUpdateServiceEvent::FirmwareWrite(value) => {
                            let offset = self.service.offset_get().unwrap();
                            defmt::info!("Writing {} bytes at offset {}", value.len(), offset);
                            self.dfu
                                .request(DfuCommand::WriteBlock(value))
                                .unwrap()
                                .await
                                .unwrap();
                            self.service.offset_set(offset + value.len() as u32).ok();
                        }
                    }
                }
            }
        }
    }
}
