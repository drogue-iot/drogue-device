pub mod remote;

use crate::flash::SharedFlash;
use crate::shared::Handle;
use core::future::Future;
use embassy_boot::FirmwareUpdater;
use embedded_storage::nor_flash::{NorFlashError, NorFlashErrorKind};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use embedded_update::{FirmwareDevice, FirmwareStatus};
use heapless::Vec;

pub const PAGE_SIZE: usize = 2048;

#[derive(Debug)]
pub enum Error {
    Flash,
    WrongOffset,
}

impl From<NorFlashErrorKind> for Error {
    fn from(_: NorFlashErrorKind) -> Self {
        Error::Flash
    }
}

pub type SharedFirmwareManager<'a, CONFIG, const MTU: usize> =
    Handle<'a, FirmwareManager<CONFIG, MTU>>;

pub trait FirmwareConfig {
    type STATE: AsyncNorFlash + AsyncReadNorFlash;
    type DFU: AsyncNorFlash;
    const BLOCK_SIZE: usize;

    fn state(&mut self) -> &mut Self::STATE;
    fn dfu(&mut self) -> &mut Self::DFU;
}

#[repr(C, align(128))]
struct Aligned([u8; PAGE_SIZE]);

/// Manages the firmware of an application using a STATE flash storage for storing
/// the state of firmware and update process, and DFU flash storage for writing the
/// firmware.
pub struct FirmwareManager<CONFIG, const MTU: usize = 16>
where
    CONFIG: FirmwareConfig,
{
    config: CONFIG,
    current_version: Vec<u8, 16>,
    next_version: Option<Vec<u8, 16>>,
    updater: FirmwareUpdater,
    buffer: Aligned,
    b_offset: usize,
    f_offset: usize,
}

impl<CONFIG, const MTU: usize> FirmwareManager<CONFIG, MTU>
where
    CONFIG: FirmwareConfig,
{
    pub fn new(config: CONFIG, updater: FirmwareUpdater, version: &[u8]) -> Self {
        Self {
            current_version: Vec::from_slice(version).unwrap(),
            next_version: Some(Vec::from_slice(b"0.2.0").unwrap()), //None,
            config,
            updater,
            buffer: Aligned([0; PAGE_SIZE]),
            b_offset: 0,
            f_offset: 0,
        }
    }

    /// Start firmware update sequence
    pub async fn start(&mut self, version: &[u8]) -> Result<(), Error> {
        self.b_offset = 0;
        self.f_offset = 0;
        self.next_version.replace(Vec::from_slice(version).unwrap());
        Ok(())
    }

    /// Mark current firmware as successfully booted
    pub async fn status(&self) -> Result<FirmwareStatus<Vec<u8, 16>>, Error> {
        Ok(FirmwareStatus {
            current_version: self.current_version.clone(),
            next_offset: self.f_offset as u32 + self.b_offset as u32,
            next_version: self.next_version.clone(),
        })
    }

    /// Mark current firmware as successfully booted
    pub async fn synced(&mut self) -> Result<(), Error> {
        let mut w = FlashWriter {
            f: self.config.state(),
        };
        self.updater
            // TODO: Support other erase sizes
            .mark_booted::<FlashWriter<'_, CONFIG::STATE, 8>>(&mut w)
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }

    /// Finish firmware update: instruct flash to swap and reset device.
    pub async fn update(&mut self, _: &[u8], _: &[u8]) -> Result<(), Error> {
        self.swap().await?;
        #[cfg(feature = "cortex_m")]
        cortex_m::peripheral::SCB::sys_reset();
        Ok(())
    }

    /// Write data to flash. Contents are not guaranteed to be written until finish is called.
    pub async fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), Error> {
        // Make sure we flush in case last write failed
        if self.b_offset == self.buffer.0.len() {
            self.flush().await?;
        }

        if self.f_offset + self.b_offset != offset as usize {
            return Err(Error::WrongOffset);
        }
        trace!("Writing {} bytes at b_offset {}", data.len(), self.b_offset);
        let mut remaining = data.len();
        while remaining > 0 {
            let to_copy = core::cmp::min(PAGE_SIZE - self.b_offset, remaining);
            let offset = data.len() - remaining;
            /*info!(
                "b_offset {}, to_copy = {}, offset {}, data len {}, remaining {}",
                self.b_offset,
                to_copy,
                offset,
                data.len(),
                remaining
            );*/

            self.buffer.0[self.b_offset..self.b_offset + to_copy]
                .copy_from_slice(&data[offset..offset + to_copy]);
            self.b_offset += to_copy;

            /*
            info!(
                "After copy, b_offset {} buffer len {}",
                self.b_offset,
                self.buffer.len()
            );*/
            if self.b_offset == self.buffer.0.len() {
                self.flush().await?;
            }
            remaining -= to_copy;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Error> {
        if self.b_offset > 0 {
            self.updater
                .write_firmware(
                    self.f_offset,
                    &self.buffer.0[..self.b_offset],
                    self.config.dfu(),
                    CONFIG::BLOCK_SIZE,
                )
                .await
                .map_err(|e| e.kind())?;
            self.f_offset += self.b_offset;
            self.b_offset = 0;
        }
        Ok(())
    }

    async fn swap(&mut self) -> Result<(), Error> {
        // Ensure buffer flushed before we
        if self.b_offset > 0 {
            for i in self.b_offset..self.buffer.0.len() {
                self.buffer.0[i] = 0;
            }
            self.b_offset = self.buffer.0.len();
            self.flush().await?;
        }

        let mut w = FlashWriter {
            f: self.config.state(),
        };
        self.updater
            // TODO: Support other erase sizes
            .update::<FlashWriter<'_, CONFIG::STATE, 8>>(&mut w)
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }
}

// Workaround for const generics
struct FlashWriter<'a, F, const WRITE_SIZE: usize> {
    f: &'a mut F,
}

impl<'a, F, const WRITE_SIZE: usize> embedded_storage::nor_flash::ErrorType
    for FlashWriter<'a, F, WRITE_SIZE>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'a,
{
    type Error = F::Error;
}

impl<'a, F, const WRITE_SIZE: usize> AsyncReadNorFlash for FlashWriter<'a, F, WRITE_SIZE>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'a,
{
    const READ_SIZE: usize = F::READ_SIZE;

    type ReadFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn read<'m>(&'m mut self, address: u32, data: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move { self.f.read(address, data).await }
    }

    fn capacity(&self) -> usize {
        self.f.capacity()
    }
}

impl<'a, F, const WRITE_SIZE: usize> AsyncNorFlash for FlashWriter<'a, F, WRITE_SIZE>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'a,
{
    const WRITE_SIZE: usize = WRITE_SIZE;
    const ERASE_SIZE: usize = F::ERASE_SIZE;

    type WriteFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn write<'m>(&'m mut self, offset: u32, data: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { self.f.write(offset, data).await }
    }

    type EraseFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn erase<'m>(&'m mut self, from: u32, to: u32) -> Self::EraseFuture<'m> {
        async move { self.f.erase(from, to).await }
    }
}

impl<CONFIG, const MTU: usize> FirmwareDevice for FirmwareManager<CONFIG, MTU>
where
    CONFIG: FirmwareConfig,
{
    const MTU: usize = MTU;
    type Version = Vec<u8, 16>;
    type Error = Error;
    type StatusFuture<'m> = impl Future<Output = Result<FirmwareStatus<Self::Version>, Error>> + 'm
    where
        Self: 'm;
    fn status<'m>(&'m mut self) -> Self::StatusFuture<'m> {
        FirmwareManager::status(self)
    }

    type StartFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn start<'m>(&'m mut self, version: &'m [u8]) -> Self::StartFuture<'m> {
        FirmwareManager::start(self, version)
    }

    type SyncedFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn synced<'m>(&'m mut self) -> Self::SyncedFuture<'m> {
        FirmwareManager::synced(self)
    }

    type UpdateFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn update<'m>(&'m mut self, version: &'m [u8], checksum: &'m [u8]) -> Self::UpdateFuture<'m> {
        FirmwareManager::update(self, version, checksum)
    }

    type WriteFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, offset: u32, data: &'m [u8]) -> Self::WriteFuture<'m> {
        FirmwareManager::write(self, offset, data)
    }
}

/// Implementation for shared resource
impl<'a, CONFIG, const MTU: usize> FirmwareDevice for SharedFirmwareManager<'a, CONFIG, MTU>
where
    CONFIG: FirmwareConfig + 'a,
{
    const MTU: usize = MTU;
    type Error = Error;
    type Version = Vec<u8, 16>;
    type StatusFuture<'m> = impl Future<Output = Result<FirmwareStatus<Self::Version>, Error>> + 'm
    where
        Self: 'm;
    fn status<'m>(&'m mut self) -> Self::StatusFuture<'m> {
        async move { self.lock().await.status().await }
    }

    type StartFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn start<'m>(&'m mut self, version: &'m [u8]) -> Self::StartFuture<'m> {
        async move { self.lock().await.start(version).await }
    }

    type SyncedFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn synced<'m>(&'m mut self) -> Self::SyncedFuture<'m> {
        async move { self.lock().await.synced().await }
    }

    type UpdateFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn update<'m>(&'m mut self, version: &'m [u8], checksum: &'m [u8]) -> Self::UpdateFuture<'m> {
        async move { self.lock().await.update(version, checksum).await }
    }

    type WriteFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, offset: u32, data: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { self.lock().await.write(offset, data).await }
    }
}

#[cfg(feature = "nrf-softdevice")]
impl FirmwareConfig for nrf_softdevice::Flash {
    type STATE = nrf_softdevice::Flash;
    type DFU = nrf_softdevice::Flash;
    const BLOCK_SIZE: usize = 4096;

    fn state(&mut self) -> &mut Self::STATE {
        self
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        self
    }
}

impl<'a, F> FirmwareConfig for SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type STATE = SharedFlash<'a, F>;
    type DFU = SharedFlash<'a, F>;
    const BLOCK_SIZE: usize = F::ERASE_SIZE;

    fn state(&mut self) -> &mut Self::STATE {
        self
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        self
    }
}
