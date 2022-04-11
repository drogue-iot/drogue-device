use core::future::Future;
use embassy::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embedded_storage::nor_flash::ErrorType;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

type DriverMutex = ThreadModeRawMutex;

pub struct FlashState<F> {
    flash: Mutex<DriverMutex, Option<F>>,
}

impl<F> FlashState<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub const fn new() -> Self {
        Self {
            flash: Mutex::new(None),
        }
    }

    pub fn initialize<'a>(&'a self, flash: F) -> SharedFlash<'a, F> {
        if let Ok(mut guard) = self.flash.try_lock() {
            guard.replace(flash);
        }
        SharedFlash {
            handle: &self.flash,
        }
    }
}

pub struct SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    handle: &'a Mutex<DriverMutex, Option<F>>,
}

unsafe impl<F> Sync for FlashState<F> where F: AsyncNorFlash + AsyncReadNorFlash {}

impl<'a, F> Clone for SharedFlash<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
        }
    }
}

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
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .read(address, data)
                .await
        }
    }

    fn capacity(&self) -> usize {
        // TODO: Fix async trait?
        self.handle.try_lock().unwrap().as_mut().unwrap().capacity()
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
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .write(offset, data)
                .await
        }
    }

    type EraseFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn erase<'m>(&'m mut self, from: u32, to: u32) -> Self::EraseFuture<'m> {
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .erase(from, to)
                .await
        }
    }
}
