use super::PAGE_SIZE;
use crate::traits::firmware::FirmwareManager;
use embedded_io::asynch::{Read, Write};
use postcard::{from_bytes, to_slice};

pub struct SerialUpdater<'a, S, F>
where
    S: Read + Write + 'static,
    F: FirmwareManager,
{
    serial: S,
    protocol: SerialUpdateProtocol<'a, F>,
}

impl<'a, S, F> SerialUpdater<'a, S, F>
where
    S: Read + Write + 'static,
    F: FirmwareManager,
{
    pub fn new(serial: S, dfu: F, version: &'a [u8]) -> Self {
        Self {
            serial,
            protocol: SerialUpdateProtocol::new(dfu, version),
        }
    }

    pub async fn run(&mut self) {
        info!("Starting serial updater");
        let mut buf = [0; FRAME_SIZE];

        let response = self.protocol.initialize();
        if let Ok(_) = to_slice(&response, &mut buf) {
            let _ = self.serial.write(&buf).await;
        } else {
            warn!("Error initializing serial");
        }

        loop {
            if let Ok(_) = self.serial.read(&mut buf[..]).await {
                let response: Result<Option<SerialResponse>, SerialError> = match from_bytes(&buf) {
                    Ok(command) => self.protocol.request(command).await,
                    Err(_e) => {
                        warn!("Error deserializing!");
                        Err(SerialError::Protocol)
                    }
                };

                if let Ok(_) = to_slice(&response, &mut buf) {
                    let _ = self.serial.write(&buf).await;
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
    F: FirmwareManager,
{
    dfu: F,
    version: &'a [u8],
}

impl<'a, F> SerialUpdateProtocol<'a, F>
where
    F: FirmwareManager,
{
    pub fn new(dfu: F, version: &'a [u8]) -> Self {
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
                self.dfu.start();
                Ok(None)
            }
            SerialCommand::Write(_, data) => {
                for chunk in data.chunks(PAGE_SIZE) {
                    match self.dfu.write(chunk).await {
                        Ok(_) => {}
                        Err(_) => return Err(SerialError::Flash),
                    }
                }
                Ok(None)
            }
            SerialCommand::Swap => match self.dfu.finish().await {
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
