#[cfg(feature = "ble+nrf52840")]
pub mod nrf52;

use core::future::Future;

pub trait Handler: Sized {
    fn handle(&self, message: heapless::Vec<u8, 384>);
}

pub trait Bearer {
    // TODO return a "stop receiving" control handle
    type ReceiveFuture<'m, H>: Future<Output = ()>
    where
        Self: 'm,
        H: 'm;

    fn start_receive<'m, H: Handler + 'm>(&'m self, handler: &'m H) -> Self::ReceiveFuture<'m, H>;

    type TransmitFuture<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn transmit<'m>(&'m self, message: &'m [u8]) -> Self::TransmitFuture<'m>;
}
