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
        self.request(QueueRequest::Enqueue(element)).await
    }

    /// Perform an _async_ dequeue. The result is available when the queue
    /// have elements to be dequeued.
    pub async fn dequeue(&self) -> Result<A::T, Error> {
        match self.request(QueueRequest::Dequeue).await {
            Ok(None) => Err(Error::Receive),
            Ok(Element(e)) => Ok(e),
            Err(e) => Err(e),
        }
    }
}

///
/// Trait that should be implemented by a Queue actors in drogue-device.
///
pub trait Queue: Actor<Request = QueueRequest<Self::T>, Response = QueueResponse> {
    type T;
}

/// Message types used by Queue implementations
#[derive(Debug)]
pub enum QueueRequest<T>
where
    T: Sized {
        Enqueue(T),
        Dequeue
    }

#[derive(Debug)]
pub enum QueueResponse<T>
where T: Sized {
    None,
    Element(T),
}
