use crate::prelude::*;

use crate::domain::time::duration::{Duration, Milliseconds};

#[derive(Debug, Clone)]
pub enum Error {
    TxInProgress,
    RxInProgress,
    TxBufferTooSmall,
    RxBufferTooSmall,
    TxBufferTooLong,
    RxBufferTooLong,
    Transmit,
    Receive,
    Timeout(usize),
    BufferNotInRAM,
}

/// API for UART access.
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
    pub async fn write<'a>(&'a self, tx_buffer: &'a [u8]) -> Result<(), Error> {
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
    pub async fn read<'a>(&'a self, rx_buffer: &'a mut [u8]) -> Result<usize, Error> {
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

///
/// Trait that should be implemented by a UART actors in drogue-device.
///
pub trait Uart: Actor {
    fn write<'a>(self, message: UartTx<'a>) -> Response<Self, Result<(), Error>>;
    fn read<'a>(self, message: UartRx<'a>) -> Response<Self, Result<usize, Error>>;
    fn read_with_timeout<'a, DUR>(
        self,
        message: UartRxTimeout<'a, DUR>,
    ) -> Response<Self, Result<usize, Error>>
    where
        DUR: Duration + Into<Milliseconds> + 'static;
}

/// Message types used by UART implementations
#[derive(Debug)]
pub struct UartTx<'a>(pub &'a [u8]);
#[derive(Debug)]
pub struct UartRx<'a>(pub &'a mut [u8]);
#[derive(Debug)]
pub struct UartRxTimeout<'a, DUR>(pub &'a mut [u8], pub DUR)
where
    DUR: Duration + Into<Milliseconds>;

/// Request handlers wrapper for the UART trait
impl<'a, A> RequestHandler<UartTx<'a>> for A
where
    A: Uart + 'static,
{
    type Response = Result<(), Error>;
    fn on_request(self, message: UartTx<'a>) -> Response<Self, Self::Response> {
        self.write(message)
    }
}

impl<'a, A> RequestHandler<UartRx<'a>> for A
where
    A: Uart + 'static,
{
    type Response = Result<usize, Error>;
    fn on_request(self, message: UartRx<'a>) -> Response<Self, Self::Response> {
        self.read(message)
    }
}

impl<'a, A, DUR> RequestHandler<UartRxTimeout<'a, DUR>> for A
where
    A: Uart + 'static,
    DUR: Duration + Into<Milliseconds> + 'static,
{
    type Response = Result<usize, Error>;
    fn on_request(self, message: UartRxTimeout<'a, DUR>) -> Response<Self, Self::Response> {
        self.read_with_timeout(message)
    }
}
