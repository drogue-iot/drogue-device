use core::future::Future;

use embedded_storage::nor_flash::NorFlashErrorKind;

pub enum Error {
    Flash,
}

impl From<NorFlashErrorKind> for Error {
    fn from(_: NorFlashErrorKind) -> Self {
        Error::Flash
    }
}

pub trait FirmwareManager {
    /// Signal firmware update start
    type StartFuture<'m>: Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn start<'m>(&'m mut self) -> Self::StartFuture<'m>;

    type MarkBootedFuture<'m>: Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    /// Mark currently running firmware as booted
    fn mark_booted<'m>(&'m mut self) -> Self::MarkBootedFuture<'m>;

    type FinishFuture<'m>: Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    /// Firmware written is finished and ready for use.
    fn finish<'m>(&'m mut self) -> Self::FinishFuture<'m>;

    /// Write data to flash. Contents are not guaranteed to be written until finish is called.
    type WriteFuture<'m>: Future<Output = Result<(), Error>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, data: &'m [u8]) -> Self::WriteFuture<'m>;
}
