use core::future::Future;
use embedded_storage::nor_flash::ErrorType;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

use crate::shared::{Handle, Shared};

pub type FlashState<F> = Shared<F>;
pub type SharedFlash<'a, F> = Handle<'a, F>;

impl<'a, F> ErrorType for SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type Error = F::Error;
}

impl<'a, F> AsyncReadNorFlash for SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    const READ_SIZE: usize = F::READ_SIZE;

    type ReadFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn read<'m>(&'m mut self, address: u32, data: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move { self.lock().await.read(address, data).await }
    }

    fn capacity(&self) -> usize {
        // TODO: Fix async trait?
        self.try_lock().unwrap().capacity()
    }
}

impl<'a, F> AsyncNorFlash for SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    const WRITE_SIZE: usize = F::WRITE_SIZE;
    const ERASE_SIZE: usize = F::ERASE_SIZE;

    type WriteFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn write<'m>(&'m mut self, offset: u32, data: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { self.lock().await.write(offset, data).await }
    }

    type EraseFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn erase<'m>(&'m mut self, from: u32, to: u32) -> Self::EraseFuture<'m> {
        async move { self.lock().await.erase(from, to).await }
    }
}
