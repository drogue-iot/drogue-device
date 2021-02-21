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
    A: UartWriter,
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
        self.request_panicking(UartWrite(tx_buffer)).await
    }
}

impl<A> Address<A>
where
    A: UartReader,
{
    /// Perform an _async_ read from the uart.
    ///
    /// # Panics
    ///
    /// While the rx_buffer may be non-static, the user must
    /// ensure that the response to the read is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn read<'a>(&'a self, rx_buffer: &'a mut [u8]) -> Result<usize, Error> {
        self.request_panicking(UartRead(rx_buffer)).await
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
        A: RequestHandler<UartReadWithTimeout<'a, DUR>, Response = Result<usize, Error>>,
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        self.request_panicking(UartReadWithTimeout(rx_buffer, timeout))
            .await
    }
}

///
/// Trait that should be implemented by a UART actors in drogue-device.
///
pub trait UartWriter: Actor {
    fn write<'a>(self, message: UartWrite<'a>) -> Response<Self, Result<(), Error>>;
}

pub trait UartReader: Actor {
    fn read<'a>(self, message: UartRead<'a>) -> Response<Self, Result<usize, Error>>;
    fn read_with_timeout<'a, DUR>(
        self,
        message: UartReadWithTimeout<'a, DUR>,
    ) -> Response<Self, Result<usize, Error>>
    where
        DUR: Duration + Into<Milliseconds> + 'static;
}

/// Message types used by UART implementations
#[derive(Debug)]
pub struct UartWrite<'a>(pub &'a [u8]);
#[derive(Debug)]
pub struct UartRead<'a>(pub &'a mut [u8]);
#[derive(Debug)]
pub struct UartReadWithTimeout<'a, DUR>(pub &'a mut [u8], pub DUR)
where
    DUR: Duration + Into<Milliseconds>;

/// Request handlers wrapper for the UART trait
impl<'a, A> RequestHandler<UartWrite<'a>> for A
where
    A: UartWriter + 'static,
{
    type Response = Result<(), Error>;
    fn on_request(self, message: UartWrite<'a>) -> Response<Self, Self::Response> {
        self.write(message)
    }
}

impl<'a, A> RequestHandler<UartRead<'a>> for A
where
    A: UartReader + 'static,
{
    type Response = Result<usize, Error>;
    fn on_request(self, message: UartRead<'a>) -> Response<Self, Self::Response> {
        self.read(message)
    }
}

impl<'a, A, DUR> RequestHandler<UartReadWithTimeout<'a, DUR>> for A
where
    A: UartReader + 'static,
    DUR: Duration + Into<Milliseconds> + 'static,
{
    type Response = Result<usize, Error>;
    fn on_request(self, message: UartReadWithTimeout<'a, DUR>) -> Response<Self, Self::Response> {
        self.read_with_timeout(message)
    }
}
