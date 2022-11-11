use {
    core::future::Future,
    embassy_boot::{AlignedBuffer, FirmwareUpdater, FirmwareWriter},
    embassy_embedded_hal::adapter::BlockingAsync,
    embedded_storage::nor_flash::{NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash},
    embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash},
    embedded_update::{FirmwareDevice, FirmwareStatus},
    heapless::Vec,
};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Flash,
    Unaligned,
    WrongOffset,
}

impl From<NorFlashErrorKind> for Error {
    fn from(_: NorFlashErrorKind) -> Self {
        Error::Flash
    }
}

pub trait FirmwareConfig {
    type STATE: AsyncNorFlash + AsyncReadNorFlash;
    type DFU: AsyncNorFlash;

    fn state(&mut self) -> &mut Self::STATE;
    fn dfu(&mut self) -> &mut Self::DFU;
}

/// Implements the embedded-update device role, which allows this to be used for any chip that supports
/// embassy-boot.
pub struct FirmwareManager<CONFIG, const WRITE_SIZE: usize = 4, const MTU: usize = 16>
where
    CONFIG: FirmwareConfig,
{
    config: CONFIG,
    current_version: Vec<u8, 16>,
    next_version: Option<Vec<u8, 16>>,
    next_offset: u32,
    updater: FirmwareUpdater,
    buffer: AlignedBuffer<WRITE_SIZE>,
    writer: Option<FirmwareWriter>,
}

impl<CONFIG, const WRITE_SIZE: usize, const MTU: usize> FirmwareManager<CONFIG, WRITE_SIZE, MTU>
where
    CONFIG: FirmwareConfig,
{
    pub fn new(config: CONFIG, updater: FirmwareUpdater, version: &[u8]) -> Self {
        Self {
            current_version: Vec::from_slice(version).unwrap(),
            next_version: None,
            next_offset: 0,
            config,
            updater,
            buffer: AlignedBuffer([0; WRITE_SIZE]),
            writer: None,
        }
    }

    /// Start firmware update sequence
    pub async fn start(&mut self, version: &[u8]) -> Result<(), Error> {
        self.next_version.replace(Vec::from_slice(version).unwrap());
        self.writer.replace(
            self.updater
                .prepare_update(self.config.dfu())
                .await
                .map_err(|_| Error::Flash)?,
        );
        Ok(())
    }

    /// Mark current firmware as successfully booted
    pub async fn status(&self) -> Result<FirmwareStatus<Vec<u8, 16>>, Error> {
        Ok(FirmwareStatus {
            current_version: self.current_version.clone(),
            next_offset: self.next_offset,
            next_version: self.next_version.clone(),
        })
    }

    /// Mark current firmware as successfully booted
    pub async fn synced(&mut self) -> Result<(), Error> {
        self.updater
            // TODO: Support other word sizes
            .mark_booted(self.config.state(), &mut self.buffer.0)
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
    ///
    /// NOTE: Make sure the length of data is a multiple of the write_size. If the length of data
    /// is less than the write_size, the data will be padded with zeros.
    pub async fn write(&mut self, mut offset: u32, data: &[u8]) -> Result<(), Error> {
        if data.len() > WRITE_SIZE && data.len() % WRITE_SIZE != 0 {
            return Err(Error::Unaligned);
        }

        if self.next_offset != offset {
            return Err(Error::WrongOffset);
        }

        trace!("Writing {} bytes at offset {}", data.len(), offset);
        if let Some(writer) = self.writer.as_mut() {
            let mut copied = 0;
            while copied < data.len() {
                let to_copy = core::cmp::min(data.len() - copied, self.buffer.0.len());
                self.buffer.0[0..to_copy].copy_from_slice(&data[copied..copied + to_copy]);
                // pad/zero
                for i in to_copy..self.buffer.0.len() {
                    self.buffer.0[i] = 0;
                }
                writer
                    .write_block(
                        offset as usize,
                        &self.buffer.0,
                        self.config.dfu(),
                        WRITE_SIZE,
                    )
                    .await
                    .map_err(|_| Error::Flash)?;
                offset += self.buffer.0.len() as u32;
                copied += to_copy;
            }
            self.next_offset = offset;
        }
        Ok(())
    }

    async fn swap(&mut self) -> Result<(), Error> {
        // Ensure we don't accidentally use the updater after this point
        self.writer.take();
        // Ensure buffer flushed before we
        self.updater
            .mark_updated(self.config.state(), &mut self.buffer.0)
            .await
            .map_err(|e| e.kind())?;
        Ok(())
    }
}

impl<CONFIG, const WRITE_SIZE: usize, const MTU: usize> FirmwareDevice
    for FirmwareManager<CONFIG, WRITE_SIZE, MTU>
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

#[cfg(feature = "nrf-softdevice")]
impl FirmwareConfig for nrf_softdevice::Flash {
    type STATE = nrf_softdevice::Flash;
    type DFU = nrf_softdevice::Flash;

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

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.flash
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.flash
    }
}
