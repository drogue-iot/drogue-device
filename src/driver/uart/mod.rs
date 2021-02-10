use crate::prelude::*;

use crate::domain::time::duration::{Duration, Milliseconds};
pub use crate::hal::uart::Error;

pub trait Uart: Actor {}

pub trait UartWriter<'a>: RequestHandler<UartTx<'a>, Response = Result<(), Error>> {}
pub trait UartReader<'a>: RequestHandler<UartRx<'a>, Response = Result<usize, Error>> {}

impl<A> Address<A>
where
    A: Uart,
{
    /// Perform an _async_ write to the uart.
    ///
    /// # Panics
    ///
    /// While the tx_buffer may be non-static, the user must
    /// ensure that the response to the write is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn write<'a>(&'a self, tx_buffer: &'a [u8]) -> Result<(), Error>
    where
        A: RequestHandler<UartTx<'a>, Response = Result<(), Error>>,
    {
        self.request_panicking(UartTx(tx_buffer)).await
    }

    /// Perform an _async_ read from the uart.
    ///
    /// # Panics
    ///
    /// While the rx_buffer may be non-static, the user must
    /// ensure that the response to the read is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn read<'a>(&'a self, rx_buffer: &'a mut [u8]) -> Result<usize, Error>
    where
        A: RequestHandler<UartRx<'a>, Response = Result<usize, Error>>,
    {
        self.request_panicking(UartRx(rx_buffer)).await
    }

    /// Perform an _async_ read from the uart with a timeout. If the request times out,
    /// the number of bytes read into the rx_buffer before the timeout will be returned.
    ///
    /// # Panics
    ///
    /// While the rx_buffer may be non-static, the user must
    /// ensure that the response to the read is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn read_with_timeout<'a, DUR>(
        &'a self,
        rx_buffer: &'a mut [u8],
        timeout: DUR,
    ) -> Result<usize, Error>
    where
        A: RequestHandler<UartRxTimeout<'a, DUR>, Response = Result<usize, Error>>,
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        self.request_panicking(UartRxTimeout(rx_buffer, timeout))
            .await
    }
}

#[derive(Debug)]
pub struct UartTx<'a>(&'a [u8]);
#[derive(Debug)]
pub struct UartRx<'a>(&'a mut [u8]);
#[derive(Debug)]
pub struct UartRxTimeout<'a, DUR>(&'a mut [u8], DUR)
where
    DUR: Duration + Into<Milliseconds>;

pub mod dma;
