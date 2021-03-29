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

impl<TX> Uart for SerialTx<TX> where TX: Write<u8> + 'static {}

impl<TX> Actor for SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    type Configuration = ();
    type Request = UartRequest<'static>;
    type Response = UartResponse;
    type DeferredFuture = DefaultDeferred<Self>;
    type ImmediateFuture = DefaultImmediate<Self>;

    fn on_request(mut self, request: UartRequest<'static>) -> Response<Self> {
        match request {
            UartRequest::Write(buf) => {
                let result = self.write_str(buf);
                Response::immediate(self, Some(result))
            }
            _ => Response::immediate(self, None),
        }
    }
}

impl<TX> SerialTx<TX>
where
    TX: Write<u8> + 'static,
{
    fn write_str(&mut self, buf: &[u8]) -> Result<usize, Error> {
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
        Ok(buf.len())
    }
}
