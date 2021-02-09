use crate::actor::{Actor, Configurable};
use crate::address::Address;
use crate::prelude::*;

pub use crate::hal::uart::Error;
use crate::hal::uart::Uart as HalUart;
use crate::interrupt::{Interrupt, InterruptContext};
use crate::package::Package;
use crate::synchronization::Signal;

use core::cell::Cell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::Nr;

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
    pub async fn read<'a>(
        &'a self,
        rx_buffer: &'a mut [u8],
        //timeout: Option<Duration>,
    ) -> Result<usize, Error>
    where
        A: RequestHandler<UartRx<'a>, Response = Result<usize, Error>>,
    {
        self.request_panicking(UartRx(rx_buffer)).await
    }
}

#[derive(Debug)]
pub struct UartTx<'a>(&'a [u8]);
#[derive(Debug)]
pub struct UartRx<'a>(&'a mut [u8]);

pub mod dma;
