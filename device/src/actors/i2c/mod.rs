use crate::traits::i2c::I2cAddress;
use crate::{Actor, Address, Inbox};
use core::future::Future;
use embedded_hal_async::i2c::*;

pub struct I2cPeripheral<I: I2c + 'static>
where
    <I as ErrorType>::Error: Send,
{
    i2c: I,
}

pub enum I2cRequest<'m> {
    Read(I2cAddress, &'m mut [u8]),
    Write(I2cAddress, &'m [u8]),
    WriteRead(I2cAddress, &'m [u8], &'m mut [u8]),
    Transaction(I2cAddress, &'m mut [embedded_hal_async::i2c::Operation<'m>]),
}
impl<I: I2c> I2cPeripheral<I>
where
    <I as ErrorType>::Error: Send,
{
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }
}

impl<I: I2c + 'static> Actor for I2cPeripheral<I>
where
    <I as ErrorType>::Error: Send,
{
    type Message<'m> = I2cRequest<'m>;

    type Response = Option<Result<(), <I as ErrorType>::Error>>;

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
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
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
                        I2cRequest::Transaction(address, operations) => {
                            let address: u8 = (*address).into();
                            self.i2c.transaction(address, operations).await
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}

impl<I: I2c<SevenBitAddress> + 'static> embedded_hal_1::i2c::ErrorType for Address<I2cPeripheral<I>>
where
    <I as ErrorType>::Error: Send,
{
    type Error = <I as ErrorType>::Error;
}

impl<I: I2c<SevenBitAddress> + 'static> I2c for Address<I2cPeripheral<I>>
where
    <I as ErrorType>::Error: Send,
{
    type ReadFuture<'a>
    where
        I: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn read<'a>(&'a mut self, address: u8, buffer: &'a mut [u8]) -> Self::ReadFuture<'a> {
        async move {
            self.request(I2cRequest::Read(address.into(), buffer))
                .unwrap()
                .await
                .unwrap()
        }
    }

    type WriteFuture<'a>
    where
        I: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;
    fn write<'a>(&'a mut self, address: u8, bytes: &'a [u8]) -> Self::WriteFuture<'a> {
        async move {
            self.request(I2cRequest::Write(address.into(), bytes))
                .unwrap()
                .await
                .unwrap()
        }
    }

    type WriteReadFuture<'a>
    where
        I: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn write_read<'a>(
        &'a mut self,
        address: u8,
        bytes: &'a [u8],
        buffer: &'a mut [u8],
    ) -> Self::WriteReadFuture<'a> {
        async move {
            self.request(I2cRequest::WriteRead(address.into(), bytes, buffer))
                .unwrap()
                .await
                .unwrap()
        }
    }

    type TransactionFuture<'a>
    where
        I: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn transaction<'a>(
        &'a mut self,
        address: u8,
        operations: &mut [embedded_hal_async::i2c::Operation<'a>],
    ) -> Self::TransactionFuture<'a> {
        let _ = address;
        let _ = operations;
        async move { todo!() }
        /*
        async move {
             * self.request(I2cRequest::Transaction(address.into(), operations))
            .unwrap()
            .await
            .unwrap()
        }
         */
    }
}
