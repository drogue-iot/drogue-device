use crate::prelude::*;

#[derive(Debug, Clone)]
pub enum Error {
    Transmit,
    Receive,
    ProducerBusy,
    ConsumerBusy,
    QueueFull,
    QueueEmpty,
}

/// API for Queues.
impl<A> Address<A>
where
    A: Queue,
{
    /// Perform an _async_ enqueue.
    ///
    /// # Panics
    ///
    /// While the tx_buffer may be non-static, the user must
    /// ensure that the response to the write is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn enqueue(&self, element: A::T) -> Result<(), Error> {
        self.request(Enqueue(element)).await
    }

    /// Perform an _async_ dequeue.
    ///
    /// # Panics
    ///
    /// While the rx_buffer may be non-static, the user must
    /// ensure that the response to the read is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn dequeue(&self) -> Result<A::T, Error> {
        self.request(Dequeue).await
    }
}

///
/// Trait that should be implemented by a Queue actors in drogue-device.
///
pub trait Queue: Actor {
    type T;
    fn enqueue(self, message: Enqueue<Self::T>) -> Response<Self, Result<(), Error>>;
    fn dequeue(self, message: Dequeue) -> Response<Self, Result<Self::T, Error>>;
}

/// Message types used by Queue implementations
#[derive(Debug)]
pub struct Enqueue<T>(pub T)
where
    T: Sized;
#[derive(Debug)]
pub struct Dequeue;

/// Request handlers wrapper for the UART trait
impl<A> RequestHandler<Enqueue<A::T>> for A
where
    A: Queue + 'static,
{
    type Response = Result<(), Error>;
    fn on_request(self, message: Enqueue<A::T>) -> Response<Self, Self::Response> {
        self.enqueue(message)
    }
}

impl<A> RequestHandler<Dequeue> for A
where
    A: Queue + 'static,
{
    type Response = Result<A::T, Error>;
    fn on_request(self, message: Dequeue) -> Response<Self, Self::Response> {
        self.dequeue(message)
    }
}
