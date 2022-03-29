use crate::{Actor, Address, Inbox};
use core::future::Future;
use embassy_boot::FirmwareUpdater;
use embedded_storage::nor_flash::{NorFlashError, NorFlashErrorKind};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

pub mod serial;

#[cfg(feature = "usb")]
pub mod usb;

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

    async fn write(&mut self, data: &[u8]) -> Result<(), F::Error> {
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
}

pub enum DfuResponse<E> {
    Ok,
    Err(E),
}

impl<E> From<Result<(), E>> for DfuResponse<E> {
    fn from(result: Result<(), E>) -> Self {
        match result {
            Ok(_) => DfuResponse::Ok,
            Err(e) => DfuResponse::Err(e),
        }
    }
}

#[cfg(feature = "defmt")]
impl<E> DfuResponse<E>
where
    E: defmt::Format,
{
    pub fn unwrap(self) -> ()
    where
        E:,
    {
        match self {
            Self::Ok => (),
            Self::Err(e) => {
                panic!("dfu error: {:?}", e);
            }
        }
    }
}

#[cfg(feature = "log")]
impl<E> DfuResponse<E>
where
    E: core::format::Debug,
{
    pub fn unwrap(self) -> ()
    where
        E:,
    {
        match self {
            Self::Ok => (),
            Self::Err(e) => {
                panic!("dfu error: {:?}", e);
            }
        }
    }
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
impl<E> DfuResponse<E> {
    pub fn unwrap(self) -> ()
    where
        E:,
    {
        match self {
            Self::Ok => (),
            Self::Err(_) => {
                panic!("dfu error")
            }
        }
    }
}

impl<E> Default for DfuResponse<E> {
    fn default() -> Self {
        Self::Ok
    }
}

pub enum DfuError {
    Other,
}

pub enum DfuCommand<'m> {
    /// Start DFU process
    Start,
    /// Write firmware block
    WriteBlock(&'m [u8]),
    /// Mark firmware write as finished and reset device
    Finish,
    /// Mark firmware as booted successfully
    Booted,
}

impl<F: AsyncNorFlash + AsyncReadNorFlash> Actor for FirmwareManager<F> {
    type Message<'m> = DfuCommand<'m>
    where
        Self: 'm;

    type Response = DfuResponse<F::Error>;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
        Self: 'm,
    {
        trace!("Starting firmware manager");
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        DfuCommand::Start => {
                            self.b_offset = 0;
                            self.f_offset = 0;
                            Ok(())
                        }
                        DfuCommand::Booted => self.updater.mark_booted(&mut self.flash).await,
                        DfuCommand::Finish => {
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
                        DfuCommand::WriteBlock(data) => self.write(data).await,
                    };
                    m.set_response(response.into());
                }
            }
        }
    }
}
