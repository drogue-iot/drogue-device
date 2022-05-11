pub mod rng;

pub use crate::drivers::ble::mesh::bearer::nrf52::rng::SoftdeviceRng;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::interface::PB_ADV_MTU;
use crate::drivers::ble::mesh::interface::{AdvertisingBearer, BearerError};
use crate::drivers::ble::mesh::{MESH_MESSAGE, PB_ADV};
use core::future::Future;
use core::mem;
use core::ptr::slice_from_raw_parts;
use heapless::Vec;
use nrf_softdevice::ble::central::{ScanConfig, ScanError};
use nrf_softdevice::ble::peripheral::AdvertiseError;
use nrf_softdevice::ble::{central, gatt_server, peripheral};
use nrf_softdevice::{raw, Flash, Softdevice};

pub struct Nrf52BleMeshFacilities {
    pub(crate) sd: &'static Softdevice,
}

impl Nrf52BleMeshFacilities {
    pub fn new(device_name: &'static str) -> Self {
        Self {
            sd: Self::new_sd(device_name),
        }
    }

    pub fn new_sd(device_name: &'static str) -> &'static Softdevice {
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
            gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
                adv_set_count: 1,
                periph_role_count: 1,
                central_role_count: 1,
                central_sec_count: 1,
                _bitfield_1: Default::default(),
            }),
            gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
                p_value: device_name.as_ptr() as *const u8 as _,
                current_len: device_name.len() as u16,
                max_len: device_name.len() as u16,
                write_perm: unsafe { mem::zeroed() },
                _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                    raw::BLE_GATTS_VLOC_STACK as u8,
                ),
            }),

            ..Default::default()
        };
        let sd = Softdevice::enable(&config);
        sd
    }

    pub fn bearer(&self) -> SoftdeviceAdvertisingBearer {
        SoftdeviceAdvertisingBearer::new(self.sd)
    }

    pub fn rng(&self) -> SoftdeviceRng {
        SoftdeviceRng::new(self.sd)
    }

    pub fn flash(&self) -> Flash {
        Flash::take(self.sd)
    }
}

pub struct SoftdeviceAdvertisingBearer {
    sd: &'static Softdevice,
}

impl SoftdeviceAdvertisingBearer {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd }
    }
}

impl AdvertisingBearer for SoftdeviceAdvertisingBearer {
    type TransmitFuture<'m> = impl Future<Output = Result<(), BearerError>> + 'm;

    fn transmit<'m>(&'m self, message: &'m Vec<u8, PB_ADV_MTU>) -> Self::TransmitFuture<'m> {
        let adv =
            peripheral::NonconnectableAdvertisement::NonscannableUndirected { adv_data: message };

        async move {
            info!("tx>");
            if let Err(err) = peripheral::advertise(
                self.sd,
                adv,
                &peripheral::Config {
                    max_events: Some(3),
                    interval: 50,
                    ..Default::default()
                },
            )
            .await
            {
                info!("tx<");
                match err {
                    AdvertiseError::Timeout => Ok(()),
                    AdvertiseError::NoFreeConn => Err(BearerError::InsufficientResources),
                    AdvertiseError::Raw(_) => Err(BearerError::TransmissionFailure),
                }
            } else {
                info!("tx<");
                Ok(())
            }
        }
    }

    type ReceiveFuture<'m> = impl Future<Output = Result<Vec<u8, PB_ADV_MTU>, BearerError>> + 'm
    where
        Self: 'm;

    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move {
            //let config = ScanConfig::default();
            let config = ScanConfig {
                active: false,
                interval: 50,
                window: 100,
                ..Default::default()
            };
            loop {
                let result = central::scan::<_, Vec<u8, PB_ADV_MTU>>(self.sd, &config, |event| {
                    let data = event.data;
                    let data = unsafe { &*slice_from_raw_parts(data.p_data, data.len as usize) };
                    if data.len() >= 2 && (data[1] == PB_ADV || data[1] == MESH_MESSAGE) {
                        Some(Vec::from_slice(data).unwrap())
                    } else {
                        None
                    }
                })
                .await;

                match result {
                    Ok(data) => {
                        return Ok(data);
                    }
                    Err(err) => {
                        match err {
                            ScanError::Timeout => { /* ignore, loop */ }
                            ScanError::Raw(_) => {
                                return Err(BearerError::Unspecified);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[nrf_softdevice::gatt_server]
pub struct ProvisioningServer {
    provisioning: ProvisioningService,
}

pub struct ProvisioningGattServer {
    sd: &'static Softdevice,
    server: ProvisioningServer,
}

impl ProvisioningGattServer {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self {
            sd,
            server: gatt_server::register(sd).unwrap(),
        }
    }

    pub async fn run(&self, uuid: Uuid) {
        // todo: more specific
        let oob: u16 = 0;
        let oob = oob.to_be_bytes();

        #[rustfmt::skip]
            let adv_data = &[
            0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
            0x03, 0x03, 0xDB, 0x2A,
            0x0a, 0x12,
            uuid.0[0],
            uuid.0[1],
            uuid.0[2],
            uuid.0[3],
            uuid.0[4],
            uuid.0[5],
            uuid.0[6],
            uuid.0[7],
            uuid.0[8],
            uuid.0[9],
            uuid.0[10],
            uuid.0[11],
            uuid.0[12],
            uuid.0[13],
            uuid.0[14],
            uuid.0[15],
            oob[0], oob[1],
        ];

        let scan_data: [u8; 0] = [];

        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data,
            scan_data: &scan_data,
        };

        let config = peripheral::Config::default();
        if let Ok(connection) = peripheral::advertise_connectable(self.sd, adv, &config).await {
            let _result = gatt_server::run(&connection, &self.server, |e| match e {
                ProvisioningServerEvent::Provisioning(_event) => {}
            })
            .await;
        }
    }
}

#[nrf_softdevice::gatt_service(uuid = "00001827-0000-1000-8000-00805f9b34fb")]
pub struct ProvisioningService {
    #[characteristic(uuid = "00002adb-0000-1000-8000-00805f9b34fb", write)]
    data_in: Vec<u8, 66>,
    #[characteristic(uuid = "00002adc-0000-1000-8000-00805f9b34fb", notify)]
    data_out: Vec<u8, 66>,
}

pub struct ProvisioningHandler<'a> {
    service: &'a ProvisioningService,
}

impl ProvisioningHandler<'_> {
    pub async fn handle(&mut self, event: ProvisioningServiceEvent) {
        match event {
            ProvisioningServiceEvent::DataInWrite(_data) => {}
            ProvisioningServiceEvent::DataOutCccdWrite { .. } => {}
        }
    }
}
