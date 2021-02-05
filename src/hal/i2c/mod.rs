use crate::prelude::*;
use core::fmt::{Formatter, LowerHex, UpperHex};
use core::marker::PhantomData;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct I2cAddress(u8);

impl I2cAddress {
    pub fn new(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for I2cAddress {
    fn into(self) -> u8 {
        self.0
    }
}

impl Into<I2cAddress> for u8 {
    fn into(self) -> I2cAddress {
        I2cAddress::new(self)
    }
}

impl LowerHex for I2cAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl UpperHex for I2cAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

pub struct I2c<I>
where
    I: 'static,
{
    peripheral: ActorContext<I2cPeripheral<I>>,
}

impl<I> I2c<I> {
    pub fn new(i2c: I) -> Self {
        Self {
            peripheral: ActorContext::new(I2cPeripheral::new(i2c)),
        }
    }
}

impl<D, I> Package<D, I2cPeripheral<I>> for I2c<I>
where
    D: Device,
{
    fn mount(
        &'static self,
        bus_address: &Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<I2cPeripheral<I>> {
        let addr = self.peripheral.mount(supervisor);
        addr
    }
}

pub struct I2cPeripheral<I> {
    i2c: I,
}

impl<I> I2cPeripheral<I> {
    fn new(i2c: I) -> Self {
        Self { i2c }
    }
}

impl<I> Actor for I2cPeripheral<I> {}

pub struct I2cRead<'b> {
    address: I2cAddress,
    buffer: &'b mut [u8],
}

impl<'b, I> RequestHandler<I2cRead<'b>> for I2cPeripheral<I>
where
    I: Read + 'static,
{
    type Response = Result<(), I::Error>;

    fn on_request(mut self, mut message: I2cRead<'b>) -> Response<Self, Self::Response> {
        let result = self.i2c.read(message.address.into(), message.buffer);
        Response::immediate(self, result)
    }
}

pub struct I2cWrite<'b> {
    address: I2cAddress,
    buffer: &'b [u8],
}

impl<'b, I> RequestHandler<I2cWrite<'b>> for I2cPeripheral<I>
where
    I: Write + 'static,
{
    type Response = Result<(), I::Error>;

    fn on_request(mut self, mut message: I2cWrite<'b>) -> Response<Self, Self::Response> {
        let result = self.i2c.write(message.address.into(), message.buffer);
        Response::immediate(self, result)
    }
}

pub struct I2cWriteRead<'b> {
    address: I2cAddress,
    bytes: &'b [u8],
    buffer: &'b mut [u8],
}

impl<'b, I> RequestHandler<I2cWriteRead<'b>> for I2cPeripheral<I>
where
    I: WriteRead + 'static,
{
    type Response = Result<(), I::Error>;

    fn on_request(mut self, mut message: I2cWriteRead<'b>) -> Response<Self, Self::Response> {
        let result = self
            .i2c
            .write_read(message.address.into(), message.bytes, message.buffer);
        Response::immediate(self, result)
    }
}

impl<I> Address<I2cPeripheral<I>>
    where
        I: Read,
{
    pub async unsafe fn read<'b>(
        &self,
        address: I2cAddress,
        bytes: &'b [u8],
        buffer: &'b mut [u8],
    ) -> Result<(), I::Error> {
        self.request_unchecked(I2cRead {
            address,
            buffer,
        }).await
    }
}

impl<I> Address<I2cPeripheral<I>>
    where
        I: Write,
{
    pub async unsafe fn write<'b>(
        &self,
        address: I2cAddress,
        buffer: &'b mut [u8],
    ) -> Result<(), I::Error> {
        self.request_unchecked(I2cWrite {
            address,
            buffer,
        }).await
    }
}

impl<I> Address<I2cPeripheral<I>>
where
    I: WriteRead,
{
    pub async unsafe fn write_read<'b>(
        &self,
        address: I2cAddress,
        bytes: &'b [u8],
        buffer: &'b mut [u8],
    ) -> Result<(), I::Error> {
        self.request_unchecked(I2cWriteRead {
            address,
            bytes,
            buffer,
        }).await
    }
}
