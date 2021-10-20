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
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}
