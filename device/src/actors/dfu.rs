use crate::{Actor, Address, Inbox};
use core::future::Future;
use embassy_boot::FirmwareUpdater;
use embedded_storage::nor_flash::ErrorType;
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

    async fn swap(&mut self, crc: u32) -> Result<(), F::Error> {
        // Ensure buffer flushed before we
        if self.b_offset > 0 {
            info!("Flushing updater");
            for i in self.b_offset..self.buffer.len() {
                self.buffer[i] = 0;
            }
            self.b_offset = self.buffer.len();
            self.flush().await?;
        }
        info!("Marking as swappable");
        self.updater.mark_update(&mut self.flash).await
    }

    async fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), F::Error> {
        info!("Writing {} bytes at {}", data.len(), offset);
        self.buffer[self.b_offset..self.b_offset + data.len()].copy_from_slice(&data);
        self.b_offset += data.len();
        if self.b_offset == self.buffer.len() {
            self.flush().await
        } else {
            Ok(())
        }
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

impl<E> DfuResponse<E> {
    pub fn unwrap(self) -> () {
        match self {
            Self::Ok => (),
            Self::Err(e) => {
                //panic!("dfu error: {:?}", e);
                panic!("dfu error");
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
    /// Write firmware to a given offset
    Write(u32, &'m [u8]),
    /// Mark firmware write as finished and compare with checksum, then reset device
    Finish(u32),
    /// Mark firmware as booted successfully
    Booted,
}

impl<F: AsyncNorFlash + AsyncReadNorFlash> Actor for FirmwareManager<F> {
    type Message<'m>
    where
        Self: 'm,
    = DfuCommand<'m>;

    type Response = DfuResponse<F::Error>;

    type OnMountFuture<'m, M>
    where
        Self: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
        Self: 'm,
    {
        info!("Starting firmware manager");
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
                        DfuCommand::Finish(crc) => {
                            let r = self.swap(*crc).await;
                            if let Ok(_) = r {
                                cortex_m::peripheral::SCB::sys_reset();
                            }
                            r
                        }
                        DfuCommand::Write(offset, data) => self.write(*offset, data).await,
                    };
                    m.set_response(response.into());
                }
            }
        }
    }
}
