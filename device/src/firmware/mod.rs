pub mod remote;

use crate::{flash::SharedFlash, shared::Handle};
use core::future::Future;
use embassy_boot::{AlignedBuffer, FirmwareUpdater};
use embassy_embedded_hal::adapter::BlockingAsync;
use embedded_storage::nor_flash::{NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use embedded_update::{FirmwareDevice, FirmwareStatus};
use heapless::Vec;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Flash,
    WrongOffset,
}

impl From<NorFlashErrorKind> for Error {
    fn from(_: NorFlashErrorKind) -> Self {
        Error::Flash
    }
}

pub type SharedFirmwareManager<'a, CONFIG, const PAGE_SIZE: usize, const MTU: usize> =
    Handle<'a, FirmwareManager<CONFIG, PAGE_SIZE, MTU>>;

pub trait FirmwareConfig {
    type STATE: AsyncNorFlash + AsyncReadNorFlash;
    type DFU: AsyncNorFlash;
    const BLOCK_SIZE: usize;

    fn state(&mut self) -> &mut Self::STATE;
    fn dfu(&mut self) -> &mut Self::DFU;
}

#[repr(C, align(128))]
struct Aligned<const PAGE_SIZE: usize>([u8; PAGE_SIZE]);

/// Manages the firmware of an application using a STATE flash storage for storing
/// the state of firmware and update process, and DFU flash storage for writing the
/// firmware.
pub struct FirmwareManager<CONFIG, const PAGE_SIZE: usize = 4096, const MTU: usize = 16>
where
    CONFIG: FirmwareConfig,
{
    config: CONFIG,
    current_version: Vec<u8, 16>,
    next_version: Option<Vec<u8, 16>>,
    updater: FirmwareUpdater,
    buffer: Aligned<PAGE_SIZE>,
    b_offset: usize,
    f_offset: usize,
}

impl<CONFIG, const PAGE_SIZE: usize, const MTU: usize> FirmwareManager<CONFIG, PAGE_SIZE, MTU>
where
    CONFIG: FirmwareConfig,
{
    pub fn new(config: CONFIG, updater: FirmwareUpdater, version: &[u8]) -> Self {
        Self {
            current_version: Vec::from_slice(version).unwrap(),
            next_version: None,
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
        let mut aligned = AlignedBuffer([0; 8]);
        self.updater
            // TODO: Support other word sizes
            .mark_booted(self.config.state(), &mut aligned.0)
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }

    /// Finish firmware update: instruct flash to swap and reset device.
    pub async fn update(&mut self, _: &[u8], _: &[u8]) -> Result<(), Error> {
        self.swap().await?;
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

        let mut aligned = AlignedBuffer([0; 8]);
        self.updater
            .mark_updated(self.config.state(), &mut aligned.0)
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }
}

impl<CONFIG, const PAGE_SIZE: usize, const MTU: usize> FirmwareDevice
    for FirmwareManager<CONFIG, PAGE_SIZE, MTU>
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
impl<'a, CONFIG, const PAGE_SIZE: usize, const MTU: usize> FirmwareDevice
    for SharedFirmwareManager<'a, CONFIG, PAGE_SIZE, MTU>
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

pub struct BlockingFlash<F: NorFlash + ReadNorFlash> {
    flash: BlockingAsync<F>,
}

impl<F: NorFlash + ReadNorFlash> BlockingFlash<F> {
    pub fn new(flash: F) -> Self {
        Self {
            flash: BlockingAsync::new(flash),
        }
    }
}

impl<F: NorFlash + ReadNorFlash> FirmwareConfig for BlockingFlash<F> {
    type STATE = BlockingAsync<F>;
    type DFU = BlockingAsync<F>;
    const BLOCK_SIZE: usize = F::ERASE_SIZE;

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.flash
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.flash
    }
}
