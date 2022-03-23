use crate::{
    actors::dfu::{DfuCommand, DfuResponse, FirmwareManager},
    Actor, Address, Inbox,
};
use core::future::Future;
use embedded_hal_async::serial::*;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use postcard::{from_bytes, to_slice};

pub struct SerialUpdater<'a, TX, RX, F>
where
    TX: Write + 'static,
    RX: Read + 'static,
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
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
    pub fn new(tx: TX, rx: RX, dfu: Address<FirmwareManager<F>>, version: &'a [u8]) -> Self {
        Self {
            tx,
            rx,
            protocol: SerialUpdateProtocol::new(dfu, version),
        }
    }
}

impl<'a, TX, RX, F> Actor for SerialUpdater<'a, TX, RX, F>
where
    TX: Write,
    RX: Read,
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
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
                    let response: Result<Option<SerialResponse>, SerialError> =
                        match from_bytes(&buf) {
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
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    dfu: Address<FirmwareManager<F>>,
    version: &'a [u8],
}

impl<'a, F> SerialUpdateProtocol<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    pub fn new(dfu: Address<FirmwareManager<F>>, version: &'a [u8]) -> Self {
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
                if let Ok(f) = self.dfu.request(DfuCommand::Start) {
                    if let DfuResponse::Ok = f.await {
                        Ok(None)
                    } else {
                        Err(SerialError::Flash)
                    }
                } else {
                    Err(SerialError::Busy)
                }
            }
            SerialCommand::Write(_, data) => {
                if let Ok(f) = self.dfu.request(DfuCommand::WriteBlock(&data[..])) {
                    if let DfuResponse::Ok = f.await {
                        Ok(None)
                    } else {
                        Err(SerialError::Flash)
                    }
                } else {
                    Err(SerialError::Busy)
                }
            }
            SerialCommand::Swap => {
                if let Ok(_) = self.dfu.notify(DfuCommand::Finish) {
                    Ok(None)
                } else {
                    Err(SerialError::Busy)
                }
            }
            SerialCommand::Sync => {
                if let Ok(f) = self.dfu.request(DfuCommand::Booted) {
                    if let DfuResponse::Ok = f.await {
                        Ok(None)
                    } else {
                        Err(SerialError::Flash)
                    }
                } else {
                    Err(SerialError::Busy)
                }
            }
        }
    }
}
