use crate::drivers::ble::mesh::bearer::{Bearer, BearerError, Handler};
use crate::drivers::ble::mesh::{MESH_MESSAGE, PB_ADV};
use core::future::Future;
use core::mem;
use core::num::NonZeroU32;
use core::ptr::slice_from_raw_parts;
use heapless::Vec;
use nrf_softdevice::ble::central::{ScanConfig, ScanError};
use nrf_softdevice::ble::peripheral::AdvertiseError;
use nrf_softdevice::ble::{central, peripheral};
use nrf_softdevice::{random_bytes, raw, Flash, Softdevice};
use rand_core::{CryptoRng, Error, RngCore};

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

#[derive(Copy, Clone)]
pub struct SoftdeviceRng {
    sd: &'static Softdevice,
}

impl SoftdeviceRng {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd }
    }
}

impl RngCore for SoftdeviceRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        loop {
            match self.try_fill_bytes(dest) {
                Ok(_) => return,
                Err(_) => {
                    // loop again
                }
            }
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        random_bytes(self.sd, dest).map_err(|_| unsafe { NonZeroU32::new_unchecked(99) }.into())
    }
}

impl CryptoRng for SoftdeviceRng {}

pub struct SoftdeviceAdvertisingBearer {
    sd: &'static Softdevice,
}

impl SoftdeviceAdvertisingBearer {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd }
    }
}

impl Bearer for SoftdeviceAdvertisingBearer {
    type TransmitFuture<'m> = impl Future<Output = Result<(), BearerError>> + 'm;

    fn transmit<'m>(&'m self, message: &'m [u8]) -> Self::TransmitFuture<'m> {
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

    type ReceiveFuture<'m, H>
    where
        Self: 'm,
        H: 'm,
    = impl Future<Output = Result<(), BearerError>> + 'm;

    fn start_receive<'m, H: Handler + 'm>(&'m self, handler: &'m H) -> Self::ReceiveFuture<'m, H> {
        async move {
            //let config = ScanConfig::default();
            let config = ScanConfig {
                active: false,
                interval: 240,
                window: 240,
                ..Default::default()
            };
            loop {
                if let Err(err) = central::scan::<_, ()>(self.sd, &config, |event| {
                    let data = event.data;
                    let data = unsafe { &*slice_from_raw_parts(data.p_data, data.len as usize) };
                    if data.len() >= 2 && (data[1] == PB_ADV || data[1] == MESH_MESSAGE) {
                        info!("rx>");
                        handler.handle(Vec::from_slice(data).unwrap());
                        info!("rx<");
                    }
                    None
                })
                .await
                {
                    match err {
                        ScanError::Timeout => {
                            // ignore, loop-de-loop
                        }
                        ScanError::Raw(_) => return Err(BearerError::Unspecified),
                    }
                }
            }
        }
    }
}
