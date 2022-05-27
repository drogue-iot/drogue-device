pub mod serial;

use crate::flash::SharedFlash;
use crate::shared::Handle;
use crate::traits::firmware::Error;
use core::future::Future;
use embassy_boot::FirmwareUpdater;
use embedded_storage::nor_flash::{NorFlashError, NorFlashErrorKind};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

pub const PAGE_SIZE: usize = 4096;

pub type SharedFirmwareManager<'a, CONFIG> = Handle<'a, FirmwareManager<CONFIG>>;

pub trait FirmwareConfig {
    type STATE: AsyncNorFlash + AsyncReadNorFlash;
    type DFU: AsyncNorFlash;
    const BLOCK_SIZE: usize;

    fn state(&mut self) -> &mut Self::STATE;
    fn dfu(&mut self) -> &mut Self::DFU;
}

#[repr(C, align(4))]
struct Aligned([u8; PAGE_SIZE]);

/// Manages the firmware of an application using a STATE flash storage for storing
/// the state of firmware and update process, and DFU flash storage for writing the
/// firmware.
pub struct FirmwareManager<CONFIG>
where
    CONFIG: FirmwareConfig,
{
    config: CONFIG,
    updater: FirmwareUpdater,
    buffer: Aligned,
    b_offset: usize,
    f_offset: usize,
}

impl<CONFIG> FirmwareManager<CONFIG>
where
    CONFIG: FirmwareConfig,
{
    pub fn new(config: CONFIG, updater: FirmwareUpdater) -> Self {
        Self {
            config,
            updater,
            buffer: Aligned([0; PAGE_SIZE]),
            b_offset: 0,
            f_offset: 0,
        }
    }

    /// Start firmware update sequence
    pub fn start(&mut self) {
        self.b_offset = 0;
        self.f_offset = 0;
    }

    /// Mark current firmware as successfully booted
    pub async fn mark_booted(&mut self) -> Result<(), NorFlashErrorKind> {
        self.updater
            .mark_booted(self.config.state())
            .await
            .map_err(|e| e.kind())
    }

    /// Finish firmware update: instruct flash to swap and reset device.
    pub async fn finish(&mut self) -> Result<(), NorFlashErrorKind> {
        self.swap().await?;
        cortex_m::peripheral::SCB::sys_reset();
    }

    /// Write data to flash. Contents are not guaranteed to be written until finish is called.
    pub async fn write(&mut self, data: &[u8]) -> Result<(), NorFlashErrorKind> {
        info!("Writing {} bytes at b_offset {}", data.len(), self.b_offset);
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

    async fn flush(&mut self) -> Result<(), NorFlashErrorKind> {
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

    async fn swap(&mut self) -> Result<(), NorFlashErrorKind> {
        // Ensure buffer flushed before we
        if self.b_offset > 0 {
            for i in self.b_offset..self.buffer.0.len() {
                self.buffer.0[i] = 0;
            }
            self.b_offset = self.buffer.0.len();
            self.flush().await?;
        }
        self.updater
            .update(self.config.state())
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }
}

impl<CONFIG> crate::traits::firmware::FirmwareManager for FirmwareManager<CONFIG>
where
    CONFIG: FirmwareConfig,
{
    type StartFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn start<'m>(&'m mut self) -> Self::StartFuture<'m> {
        async move {
            FirmwareManager::start(self);
            Ok(())
        }
    }

    type MarkBootedFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn mark_booted<'m>(&'m mut self) -> Self::MarkBootedFuture<'m> {
        async move {
            FirmwareManager::mark_booted(self).await?;
            Ok(())
        }
    }

    type FinishFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn finish<'m>(&'m mut self) -> Self::FinishFuture<'m> {
        async move {
            FirmwareManager::finish(self).await?;
            Ok(())
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, data: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            FirmwareManager::write(self, data).await?;
            Ok(())
        }
    }
}

/// Implementation for shared resource
impl<'a, CONFIG> crate::traits::firmware::FirmwareManager for SharedFirmwareManager<'a, CONFIG>
where
    CONFIG: FirmwareConfig + 'a,
{
    type StartFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn start<'m>(&'m mut self) -> Self::StartFuture<'m> {
        async move {
            self.lock().await.start();
            Ok(())
        }
    }

    type MarkBootedFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn mark_booted<'m>(&'m mut self) -> Self::MarkBootedFuture<'m> {
        async move {
            self.lock().await.mark_booted().await?;
            Ok(())
        }
    }

    type FinishFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn finish<'m>(&'m mut self) -> Self::FinishFuture<'m> {
        async move {
            self.lock().await.finish().await?;
            Ok(())
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, data: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.lock().await.write(data).await?;
            Ok(())
        }
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
