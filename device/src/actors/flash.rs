use crate::{Actor, Address, Inbox};
use core::future::Future;
use embedded_storage::nor_flash::ErrorType;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

pub struct SharedFlash<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    flash: F,
}

impl<F> SharedFlash<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(flash: F) -> Self {
        Self { flash }
    }
}

pub enum FlashOp<'m> {
    Write(u32, &'m [u8]),
    Erase(u32, u32),
    Read(u32, &'m mut [u8]),
    Capacity(&'m mut usize),
}

impl<F> Actor for SharedFlash<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type Message<'m>
    where
        Self: 'm,
    = FlashOp<'m>;
    type Response = Option<Result<(), F::Error>>;

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
        async move {
            info!("Starting shared flash manager");
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        FlashOp::Write(offset, buf) => self.flash.write(*offset, buf).await,
                        FlashOp::Erase(from, to) => self.flash.erase(*from, *to).await,
                        FlashOp::Read(offset, buf) => self.flash.read(*offset, buf).await,
                        FlashOp::Capacity(cap) => {
                            **cap = self.flash.capacity();
                            Ok(())
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}

impl<F> From<Address<SharedFlash<F>>> for SharedFlashHandle<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    fn from(address: Address<SharedFlash<F>>) -> Self {
        Self(address)
    }
}

pub struct SharedFlashHandle<F>(pub Address<SharedFlash<F>>)
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static;

impl<F> ErrorType for SharedFlashHandle<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type Error = F::Error;
}

impl<F> AsyncReadNorFlash for SharedFlashHandle<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    const READ_SIZE: usize = F::READ_SIZE;

    type ReadFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a;
    fn read<'a>(&'a mut self, address: u32, data: &'a mut [u8]) -> Self::ReadFuture<'a> {
        async move {
            self.0
                .request(FlashOp::Read(address as u32, data))
                .unwrap()
                .await
                .unwrap()
        }
    }

    fn capacity(&self) -> usize {
        // TODO: Fix async trait?
        todo!()
    }
}

impl<F> AsyncNorFlash for SharedFlashHandle<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    const WRITE_SIZE: usize = F::WRITE_SIZE;
    const ERASE_SIZE: usize = F::ERASE_SIZE;

    type WriteFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a;
    fn write<'a>(&'a mut self, offset: u32, data: &'a [u8]) -> Self::WriteFuture<'a> {
        async move {
            self.0
                .request(FlashOp::Write(offset, data))
                .unwrap()
                .await
                .unwrap()
        }
    }

    type EraseFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a;
    fn erase<'a>(&'a mut self, from: u32, to: u32) -> Self::EraseFuture<'a> {
        async move {
            self.0
                .request(FlashOp::Erase(from, to))
                .unwrap()
                .await
                .unwrap()
        }
    }
}
