use crate::{Actor, Address, Inbox};
use core::future::Future;
use embassy_boot::FirmwareUpdater;
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

    async fn swap(&mut self) -> Result<(), F::Error> {
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

    async fn write(&mut self, data: &[u8]) -> Result<(), F::Error> {
        info!("Writing {} bytes", data.len());
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
                        DfuCommand::Finish => {
                            let r = self.swap().await;
                            if let Ok(_) = r {
                                info!("Resetting device");
                                cortex_m::peripheral::SCB::sys_reset();
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
