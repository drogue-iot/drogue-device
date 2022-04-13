pub mod serial;

use crate::shared::Handle;
use crate::traits::firmware::Error;
use core::future::Future;
use embassy_boot::FirmwareUpdater;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

pub const PAGE_SIZE: usize = 4096;

pub type SharedFirmwareManager<'a, F> = Handle<'a, FirmwareManager<F>>;

pub struct FirmwareManager<F: AsyncNorFlash + AsyncReadNorFlash> {
    flash: F,
    updater: FirmwareUpdater,
    buffer: [u8; PAGE_SIZE],
    b_offset: usize,
    f_offset: usize,
}

impl<F: AsyncNorFlash + AsyncReadNorFlash> FirmwareManager<F> {
    pub fn new(flash: F, updater: FirmwareUpdater) -> Self {
        Self {
            flash,
            updater,
            buffer: [0; PAGE_SIZE],
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
    pub async fn mark_booted(&mut self) -> Result<(), F::Error> {
        self.updater.mark_booted(&mut self.flash).await
    }

    /// Finish firmware update: instruct flash to swap and reset device.
    pub async fn finish(&mut self) -> Result<(), F::Error> {
        self.swap().await?;
        cortex_m::peripheral::SCB::sys_reset();
    }

    /// Write data to flash. Contents are not guaranteed to be written until finish is called.
    pub async fn write(&mut self, data: &[u8]) -> Result<(), F::Error> {
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
            self.buffer[self.b_offset..self.b_offset + to_copy]
                .copy_from_slice(&data[offset..offset + to_copy]);
            self.b_offset += to_copy;

            /*
            info!(
                "After copy, b_offset {} buffer len {}",
                self.b_offset,
                self.buffer.len()
            );*/
            if self.b_offset == self.buffer.len() {
                self.flush().await?;
            }
            remaining -= to_copy;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), F::Error> {
        if self.b_offset > 0 {
            self.updater
                .write_firmware(
                    self.f_offset,
                    &self.buffer[..self.b_offset],
                    &mut self.flash,
                )
                .await?;
            self.f_offset += self.b_offset;
            self.b_offset = 0;
        }
        Ok(())
    }

    async fn swap(&mut self) -> Result<(), F::Error> {
        // Ensure buffer flushed before we
        if self.b_offset > 0 {
            for i in self.b_offset..self.buffer.len() {
                self.buffer[i] = 0;
            }
            self.b_offset = self.buffer.len();
            self.flush().await?;
        }
        self.updater.mark_update(&mut self.flash).await
    }
}

impl<F: AsyncNorFlash + AsyncReadNorFlash> crate::traits::firmware::FirmwareManager
    for FirmwareManager<F>
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
impl<'a, F: AsyncNorFlash + AsyncReadNorFlash> crate::traits::firmware::FirmwareManager
    for SharedFirmwareManager<'a, F>
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
