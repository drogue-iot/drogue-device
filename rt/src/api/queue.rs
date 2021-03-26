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
impl<A, T> Address<A>
where
    A: Queue<T>,
    T: Sized,
{
    /// Perform an _async_ enqueue.
    pub async fn enqueue(&self, element: T) -> Result<(), Error> {
        self.request(QueueRequest::Enqueue(element)).await
    }

    /// Perform an _async_ dequeue. The result is available when the queue
    /// have elements to be dequeued.
    pub async fn dequeue(&self) -> Result<T, Error> {
        match self.request(QueueRequest::Dequeue).await {
            Ok(None) => Err(Error::Receive),
            Ok(QueueResponse::Element(e)) => Ok(e),
            Err(e) => Err(e),
        }
    }
}

///
/// Trait that should be implemented by a Queue actors in drogue-device.
///
pub trait Queue<T>: Actor<Request = QueueRequest<T>, Response = QueueResponse<T>> {}

/// Message types used by Queue implementations
#[derive(Debug)]
pub enum QueueRequest<T>
where
    T: Sized,
{
    Enqueue(T),
    Dequeue,
}

#[derive(Debug)]
pub enum QueueResponse<T>
where
    T: Sized,
{
    None,
    Element(T),
}
