use embassy_boot::FirmwareUpdater;
use embedded_storage::nor_flash::{NorFlashError, NorFlashErrorKind};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

pub const PAGE_SIZE: usize = 4096;

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
    pub async fn start(&mut self) {
        self.b_offset = 0;
        self.f_offset = 0;
    }

    /// Mark current firmware as successfully booted
    pub async fn mark_booted(&mut self) -> Result<(), F::Error> {
        self.updater.mark_booted(&mut self.flash).await
    }

    /// Finish firmware update: instruct flash to swap and reset device.
    pub async fn finish(&mut self) -> Result<(), F::Error> {
        let r = self.swap().await;
        match r {
            Ok(_) => {
                trace!("Resetting device");
                cortex_m::peripheral::SCB::sys_reset();
            }
            Err(ref e) => match e.kind() {
                NorFlashErrorKind::Other => {
                    trace!("FlashError::Other");
                }
                NorFlashErrorKind::NotAligned => {
                    trace!("FlashError::NotAligned");
                }
                NorFlashErrorKind::OutOfBounds => {
                    trace!("FlashError::OutOfBounds");
                }
                _ => {
                    trace!("Unknown error");
                }
            },
        }
        r
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
