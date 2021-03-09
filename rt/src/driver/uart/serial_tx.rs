use crate::api::uart::*;
use crate::driver::uart::serial_rx::SerialData;
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
    fn write<'a>(mut self, message: UartWrite<'a>) -> Response<Self, Result<(), Error>> {
        let buf = message.0;
        let result = self.write_str(message.0);
        Response::immediate(self, result)
    }
}

impl<TX> NotifyHandler<SerialData> for SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    fn on_notify(mut self, message: SerialData) -> Completion<Self> {
        let mut d = [0; 1];
        d[0] = message.0;
        let r = self.write_str(&d[..]);
        Completion::immediate(self)
    }
}
