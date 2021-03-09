use crate::api::uart::*;
use crate::prelude::*;

use embedded_hal::serial::Write;

pub struct SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    tx: TX,
}

impl<TX> SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    pub fn new(tx: TX) -> Self {
        Self { tx }
    }
}

impl<TX> Actor for SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    type Configuration = ();
}

impl<TX> SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    fn write_str(&mut self, buf: &[u8]) -> Result<(), Error> {
        for b in buf.iter() {
            loop {
                match self.tx.write(*b) {
                    Err(nb::Error::WouldBlock) => {
                        nb::block!(self.tx.flush()).map_err(|_| Error::Transmit)?;
                    }
                    Err(_) => return Err(Error::Transmit),
                    _ => break,
                }
            }
        }
        nb::block!(self.tx.flush()).map_err(|_| Error::Transmit)?;
        Ok(())
    }
}

impl<TX> UartWriter for SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    fn write(mut self, message: UartWrite<'_>) -> Response<Self, Result<(), Error>> {
        let buf = message.0;
        let result = self.write_str(message.0);
        Response::immediate(self, result)
    }
}
