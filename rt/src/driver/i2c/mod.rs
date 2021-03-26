use crate::api::i2c::I2cAddress;
use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

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

impl<I> Package for I2c<I> {
    type Primary = I2cPeripheral<I>;
    type Configuration = ();

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        self.peripheral.mount((), supervisor)
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.peripheral.address()
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

pub enum I2cRequest<'b> {
    Read(I2cRead<'b>),
    Write(I2cWrite<'b>),
    WriteRead(I2cWriteRead<'b>),
}

#[derive(Debug)]
pub struct I2cRead<'b> {
    address: I2cAddress,
    buffer: &'b mut [u8],
}

#[derive(Debug)]
pub struct I2cWrite<'b> {
    address: I2cAddress,
    buffer: &'b [u8],
}

#[derive(Debug)]
pub struct I2cWriteRead<'b> {
    address: I2cAddress,
    bytes: &'b [u8],
    buffer: &'b mut [u8],
}

impl<'b, I> Actor for I2cPeripheral<I> {
    type Configuration = ();
    type Request = I2cRequest<'b>;
    type Response = Result<(), I::Error>;

    fn on_request(mut self, message: I2cRequest<'b>) -> Response<Self> {
        let result = match message {
            I2cRequest::Write(message) => self.i2c.write(message.address.into(), message.buffer),
            I2cRequest::Read(message) => self.i2c.read(message.address.into(), message.buffer),
            I2cRequest::WriteRead(message) => {
                self.i2c
                    .write_read(message.address.into(), message.bytes, message.buffer)
            }
        };
        Response::immediate(self, result)
    }
}

impl<I> Address<I2cPeripheral<I>>
where
    I: Read,
{
    /// # Panics
    /// The future *must* be fully `.await`'d before allowing the `bytes` or `buffer` arguments to fall out of scope, otherwise a panic will occur.
    pub async fn read(&self, address: I2cAddress, buffer: &mut [u8]) -> Result<(), I::Error> {
        self.request_panicking(I2cRead { address, buffer }).await
    }
}

impl<I> Address<I2cPeripheral<I>>
where
    I: Write,
{
    /// # Panics
    /// The future *must* be fully `.await`'d before allowing the `buffer` argument to fall out of scope, otherwise a panic will occur.
    pub async fn write(&self, address: I2cAddress, buffer: &[u8]) -> Result<(), I::Error> {
        self.request_panicking(I2cWrite { address, buffer }).await
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
        self.request_panicking(I2cWriteRead {
            address,
            bytes,
            buffer,
        })
        .await
    }
}
