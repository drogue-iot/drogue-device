use crate::{
    actors::dfu::{DfuCommand, FirmwareManager},
    Address,
};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use heapless::Vec;

// The FirmwareUpdate proprietary GATT service
#[nrf_softdevice::gatt_service(uuid = "1861")]
pub struct FirmwareService {
    /// Firmware data to be written
    #[characteristic(uuid = "1234", write)]
    firmware: Vec<u8, 64>,

    /// Current write offset
    #[characteristic(uuid = "1235", read)]
    offset: u32,

    /// State control
    #[characteristic(uuid = "1236", write)]
    control: u8,

    /// Version of current running firmware
    #[characteristic(uuid = "2a26", read)]
    version: Vec<u8, 16>,

    /// Next version being written
    #[characteristic(uuid = "1238", write, read)]
    next_version: Vec<u8, 16>,

    /// Max firmware block size for device
    #[characteristic(uuid = "1239", read)]
    mtu: u8,
}

pub struct FirmwareGattService<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    service: &'a FirmwareService,
    dfu: Address<FirmwareManager<F>>,
}

impl<'a, F> FirmwareGattService<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(
        service: &'a FirmwareService,
        dfu: Address<FirmwareManager<F>>,
        version: &[u8],
    ) -> Result<Self, ()> {
        service
            .version_set(Vec::from_slice(version)?)
            .map_err(|_| ())?;
        Ok(Self { service, dfu })
    }

    pub async fn handle(&self, event: &FirmwareServiceEvent) {
        match event {
            FirmwareServiceEvent::ControlWrite(value) => {
                debug!("Write firmware control: {}", value);
                if *value == 1 {
                    self.service.offset_set(0).ok();
                    self.dfu.request(DfuCommand::Start).unwrap().await.unwrap();
                } else if *value == 2 {
                    self.dfu.notify(DfuCommand::Finish).unwrap();
                } else if *value == 3 {
                    self.dfu.request(DfuCommand::Booted).unwrap().await.unwrap();
                }
            }
            FirmwareServiceEvent::FirmwareWrite(value) => {
                let offset = self.service.offset_get().unwrap();
                self.dfu
                    .request(DfuCommand::WriteBlock(value))
                    .unwrap()
                    .await
                    .unwrap();
                self.service.offset_set(offset + value.len() as u32).ok();
            }
            _ => {}
        }
    }
}
