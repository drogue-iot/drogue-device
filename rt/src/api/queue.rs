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
    pub async fn enqueue(&self, element: A::T) -> Result<(), Error> {
        self.request(Enqueue(element)).await
    }

    /// Perform an _async_ dequeue. The result is available when the queue
    /// have elements to be dequeued.
    pub async fn dequeue(&self) -> Result<A::T, Error> {
        self.request(Dequeue).await
    }

    /// Perform an _async_ dequeue attempt. If there are no elements in the queue,
    /// no element is returned.
    pub async fn try_dequeue(&self) -> Option<A::T> {
        self.request(TryDequeue).await
    }
}

///
/// Trait that should be implemented by a Queue actors in drogue-device.
///
pub trait Queue: Actor {
    type T;
    fn enqueue(self, message: Enqueue<Self::T>) -> Response<Self, Result<(), Error>>;
    fn dequeue(self, message: Dequeue) -> Response<Self, Result<Self::T, Error>>;
    fn try_dequeue(self, message: TryDequeue) -> Response<Self, Option<Self::T>>;
}

/// Message types used by Queue implementations
#[derive(Debug)]
pub struct Enqueue<T>(pub T)
where
    T: Sized;
#[derive(Debug)]
pub struct Dequeue;

#[derive(Debug)]
pub struct TryDequeue;

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

impl<A> RequestHandler<TryDequeue> for A
where
    A: Queue + 'static,
{
    type Response = Option<A::T>;
    fn on_request(self, message: TryDequeue) -> Response<Self, Self::Response> {
        self.try_dequeue(message)
    }
}
