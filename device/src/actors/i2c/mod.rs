use crate::traits::i2c::I2cAddress;
use crate::{Actor, Address, Inbox};
use core::future::Future;
use embassy::traits::i2c::*;

pub struct I2cPeripheral<I: I2c + 'static>
where
    <I as I2c>::Error: Send,
{
    i2c: I,
}

pub enum I2cRequest<'m> {
    Read(I2cAddress, &'m mut [u8]),
    Write(I2cAddress, &'m [u8]),
    WriteRead(I2cAddress, &'m [u8], &'m mut [u8]),
}
impl<I: I2c> I2cPeripheral<I>
where
    <I as I2c>::Error: Send,
{
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }
}

impl<I: I2c + 'static> Actor for I2cPeripheral<I>
where
    <I as I2c>::Error: Send,
{
    type Message<'m> = I2cRequest<'m>;

    type Response = Option<Result<(), I::Error>>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                let mut m = inbox.next().await;
                let response = match m.message() {
                    I2cRequest::Read(address, buffer) => {
                        let address: u8 = (*address).into();
                        self.i2c.read(address, buffer).await
                    }
                    I2cRequest::Write(address, bytes) => {
                        let address: u8 = (*address).into();
                        self.i2c.write(address, bytes).await
                    }
                    I2cRequest::WriteRead(address, bytes, buffer) => {
                        let address: u8 = (*address).into();
                        self.i2c.write_read(address, bytes, buffer).await
                    }
                };
                m.set_response(Some(response));
            }
        }
    }
}

#[rustfmt::skip]
impl<I: I2c<SevenBitAddress> + 'static> I2c for Address<'static, I2cPeripheral<I>>
where
    <I as I2c>::Error: Send,
{
    type Error = I::Error;

    type WriteFuture<'a> where I: 'a = impl Future<Output = Result<(), Self::Error>> + 'a;
    type ReadFuture<'a> where I: 'a = impl Future<Output = Result<(), Self::Error>> + 'a;
    type WriteReadFuture<'a> where I: 'a = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn read<'a>(&'a mut self, address: u8, buffer: &'a mut [u8]) -> Self::ReadFuture<'a> {
        async move {
            self.request(I2cRequest::Read(address.into(), buffer)).unwrap().await.unwrap()
        }
    }

    fn write<'a>(&'a mut self, address: u8, bytes: &'a [u8]) -> Self::WriteFuture<'a> {
        async move {
            self.request(I2cRequest::Write(address.into(), bytes)).unwrap().await.unwrap()
        }
    }

    fn write_read<'a>(
        &'a mut self,
        address: u8,
        bytes: &'a [u8],
        buffer: &'a mut [u8],
    ) -> Self::WriteReadFuture<'a> {
        async move {
            self.request(I2cRequest::WriteRead(address.into(), bytes, buffer)).unwrap().await.unwrap()
        }
    }

}
