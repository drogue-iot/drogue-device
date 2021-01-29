//! Traits and types for notify, request and event handlers.


use core::future::Future;

use crate::prelude::Actor;
use crate::alloc::{alloc, Box};

/// Return value from a `RequestHandler` to allow for synchronous or
/// asynchronous handling of the request.
///
/// *Note:* It is generally better and easier to use the associated
/// functions to construct instances of `Response<T>` than to attempt
/// creating them directly.
pub enum Response<T> {
    /// See `immediate(val)`.
    Immediate(T),

    /// See `defer(future)`.
    Defer(Box<dyn Future<Output = T>>),

    /// See `immediate_future(future)`.
    ImmediateFuture(Box<dyn Future<Output = T>>),
}

impl<T> Response<T> {

    /// Return an immediate value, synchronously, as the response
    /// to the request.
    pub fn immediate(val: T) -> Self {
        Self::Immediate(val)
    }

    /// Return a value asynchornously using the supplied future
    /// within the context of *this* actor to calculate the value.
    pub fn defer<F: Future<Output = T> + 'static>(f: F) -> Self
    where
        T: 'static,
    {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }

    /// Return an immediate future, synchronously, which will be
    /// executed asynchronously within the *requester's* context
    /// before the original `.request(...).await` completes.
    pub fn immediate_future<F: Future<Output = T> + 'static>(f: F) -> Self
    where
        T: 'static,
    {
        Self::ImmediateFuture(Box::new(alloc(f).unwrap()))
    }
}

/// Trait denoting the capability to respond to an asynchronous request.
pub trait RequestHandler<M>
where
    Self: Actor + Sized,
{
    /// The response type.
    type Response: 'static;

    /// Response to the request.
    fn on_request(&'static mut self, message: M) -> Response<Self::Response>;
}

/// Return value from a `NotifyHandler` to allow for immediate synchronous handling
/// of the notification or asynchronous handling.
pub enum Completion {

    /// See `immediate()`
    Immediate(),

    /// See `defer(future)`
    Defer(Box<dyn Future<Output = ()>>),
}

impl Completion {

    /// Indicates the notification has been immediately handled.
    pub fn immediate() -> Self {
        Self::Immediate()
    }

    /// Provide a future for asynchronous handling of the notification
    /// within this actor's context.
    pub fn defer<F: Future<Output = ()> + 'static>(f: F) -> Self {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }
}


/// Trait denoting the capability of being notified.
pub trait NotifyHandler<M>
where
    Self: Sized,
{
    /// Handle the notification.
    fn on_notify(&'static mut self, message: M) -> Completion;
}


/// Trait to be implemented by a `Device` implementation in order to receive
/// messages for the `EventBus`.
///
/// Actors desiring to publish messages may constrain their own generic
/// `<D:Device>` parameters with `+ EventHandler<MyEventType>` for whatever
/// events they wish to emit.
pub trait EventHandler<E> {

    /// Receive an event message for the bus.
    ///
    /// This should be a non-blocked synchronous blob of logic, usually
    /// simply routing and adapting messages to be sent along to other
    /// actors held by the device.
    ///
    /// The default implementation simply drops the event.
    fn on_event(&'static mut self, event: E) {

    }
}
