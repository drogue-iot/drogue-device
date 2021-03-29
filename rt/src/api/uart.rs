use crate::prelude::*;

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

pub type UartResponse = Option<Result<usize, Error>>;

/// API for UART access.
impl<A> Address<A>
where
    A: Actor<Request = UartRequest<'static>, Response = UartResponse>,
{
    /// Perform an _async_ write to the uart.
    ///
    /// # Panics
    ///
    /// While the tx_buffer may be non-static, the user must
    /// ensure that the response to the write is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn write<'a>(&'a self, tx_buffer: &'a [u8]) -> Result<usize, Error> {
        let req = UartRequest::Write(tx_buffer);
        // TODO: Generic associcated types...
        let req = unsafe { core::mem::transmute::<_, UartRequest<'static>>(req) };
        self.request_panicking(req).await.unwrap()
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
        let req = UartRequest::Read(rx_buffer);
        let req = unsafe { core::mem::transmute::<_, UartRequest<'static>>(req) };
        self.request_panicking(req).await.unwrap()
    }
}

///
/// Trait that should be implemented by a UART actors in drogue-device.
///
pub trait Uart: Actor<Request = UartRequest<'static>, Response = UartResponse> {}

pub enum UartRequest<'a> {
    Write(&'a [u8]),
    Read(&'a mut [u8]),
}
