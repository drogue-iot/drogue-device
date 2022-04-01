use super::FirmwareManager;
use embedded_hal_async::serial::*;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use postcard::{from_bytes, to_slice};

pub struct SerialUpdater<'a, TX, RX, F>
where
    TX: Write + 'static,
    RX: Read + 'static,
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    tx: TX,
    rx: RX,
    protocol: SerialUpdateProtocol<'a, F>,
}

impl<'a, TX, RX, F> SerialUpdater<'a, TX, RX, F>
where
    TX: Write,
    RX: Read,
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(tx: TX, rx: RX, dfu: FirmwareManager<F>, version: &'a [u8]) -> Self {
        Self {
            tx,
            rx,
            protocol: SerialUpdateProtocol::new(dfu, version),
        }
    }

    pub async fn run(&mut self) {
        info!("Starting serial updater");
        let mut buf = [0; FRAME_SIZE];

        let response = self.protocol.initialize();
        if let Ok(_) = to_slice(&response, &mut buf) {
            let _ = self.tx.write(&buf).await;
        } else {
            warn!("Error initializing serial");
        }

        loop {
            if let Ok(_) = self.rx.read(&mut buf[..]).await {
                let response: Result<Option<SerialResponse>, SerialError> = match from_bytes(&buf) {
                    Ok(command) => self.protocol.request(command).await,
                    Err(_e) => {
                        warn!("Error deserializing!");
                        Err(SerialError::Protocol)
                    }
                };

                if let Ok(_) = to_slice(&response, &mut buf) {
                    let _ = self.tx.write(&buf).await;
                } else {
                    warn!("Error serializing response");
                }
            }
        }
    }
}

/// Defines a serial protocol for DFU
use serde::{Deserialize, Serialize};
pub const FRAME_SIZE: usize = 1024;

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SerialCommand<'a> {
    Version,
    Start,
    Write(u32, &'a [u8]),
    Swap,
    Sync,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SerialResponse<'a> {
    Version(&'a [u8]),
}

#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SerialError {
    Flash,
    Busy,
    Memory,
    Protocol,
}

pub struct SerialUpdateProtocol<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    dfu: FirmwareManager<F>,
    version: &'a [u8],
}

impl<'a, F> SerialUpdateProtocol<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(dfu: FirmwareManager<F>, version: &'a [u8]) -> Self {
        Self { dfu, version }
    }

    pub fn initialize(&self) -> Result<Option<SerialResponse<'_>>, SerialError> {
        Ok(Some(SerialResponse::Version(self.version)))
    }

    pub async fn request(
        &mut self,
        command: SerialCommand<'_>,
    ) -> Result<Option<SerialResponse<'_>>, SerialError> {
        match command {
            SerialCommand::Version => Ok(Some(SerialResponse::Version(self.version))),
            SerialCommand::Start => {
                self.dfu.start().await;
                Ok(None)
            }
            SerialCommand::Write(_, data) => match self.dfu.write(data).await {
                Ok(_) => Ok(None),
                Err(_) => Err(SerialError::Flash),
            },
            SerialCommand::Swap => match self.dfu.swap().await {
                Ok(_) => Ok(None),
                Err(_) => Err(SerialError::Flash),
            },
            SerialCommand::Sync => match self.dfu.mark_booted().await {
                Ok(_) => Ok(None),
                Err(_) => Err(SerialError::Flash),
            },
        }
    }
}
