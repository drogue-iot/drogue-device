use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use crate::hal::i2c::I2cAddress;

pub struct I2c<I>
where
    I: 'static,
{
    peripheral: ActorContext<I2cPeripheral<I>>,
}

impl<I> I2c<I> {
    pub fn new(i2c: I) -> Self {
        Self {
            peripheral: ActorContext::new(I2cPeripheral::new(i2c)).with_name("i2c"),
        }
    }
}

impl<D, I> Package<D, I2cPeripheral<I>> for I2c<I>
where
    D: Device,
{
    fn mount(
        &'static self,
        bus_address: Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<I2cPeripheral<I>> {
        self.peripheral.mount(supervisor)
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

#[derive(Debug)]
pub struct I2cRead<'b> {
    address: I2cAddress,
    buffer: &'b mut [u8],
}

impl<'b, I> RequestHandler<I2cRead<'b>> for I2cPeripheral<I>
where
    I: Read + 'static,
{
    type Response = Result<(), I::Error>;

    fn on_request(mut self, message: I2cRead<'b>) -> Response<Self, Self::Response> {
        let result = self.i2c.read(message.address.into(), message.buffer);
        Response::immediate(self, result)
    }
}


#[derive(Debug)]
pub struct I2cWrite<'b> {
    address: I2cAddress,
    buffer: &'b [u8],
}

impl<'b, I> RequestHandler<I2cWrite<'b>> for I2cPeripheral<I>
where
    I: Write + 'static,
{
    type Response = Result<(), I::Error>;

    fn on_request(mut self, message: I2cWrite<'b>) -> Response<Self, Self::Response> {
        let result = self.i2c.write(message.address.into(), message.buffer);
        Response::immediate(self, result)
    }
}

#[derive(Debug)]
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

    fn on_request(mut self, message: I2cWriteRead<'b>) -> Response<Self, Self::Response> {
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
    /// # Panics
    /// The future *must* be fully `.await`'d before allowing the `bytes` or `buffer` arguments to fall out of scope, otherwise a panic will occur.
    pub async fn read(
        &self,
        address: I2cAddress,
        buffer: &mut [u8],
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
    /// # Panics
    /// The future *must* be fully `.await`'d before allowing the `buffer` argument to fall out of scope, otherwise a panic will occur.
    pub async fn write(
        &self,
        address: I2cAddress,
        buffer: &[u8],
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
    /// # Panics
    /// The future *must* be fully `.await`'d before allowing the `bytes` and `buffer` arguments to fall out of scope, otherwise a panic will occur.
    pub async fn write_read<'b>(
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
