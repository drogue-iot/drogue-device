//! Traits and types for notify, request and event handlers.

use core::future::Future;

use crate::alloc::{alloc, Box};
use crate::prelude::Actor;

/// Return value from a `RequestHandler` to allow for synchronous or
/// asynchronous handling of the request.
///
/// *Note:* It is generally better and easier to use the associated
/// functions to construct instances of `Response<T>` than to attempt
/// creating them directly.
pub enum Response<A: Actor + 'static, T> {
    /// See `immediate(val)`.
    Immediate(&'static mut A, T),

    /// See `defer(future)`.
    Defer(Box<dyn Future<Output = (&'static mut A, T)>>),

    /// See `immediate_future(future)`.
    ImmediateFuture(&'static mut A, Box<dyn Future<Output = T>>),
}

impl<T, A: Actor + 'static> Response<A, T> {
    /// Return an immediate value, synchronously, as the response
    /// to the request.
    pub fn immediate(actor: &'static mut A, val: T) -> Self {
        Self::Immediate(actor, val)
    }

    /// Return a value asynchornously using the supplied future
    /// within the context of *this* actor to calculate the value.
    pub fn defer<F: Future<Output = (&'static mut A, T)> + 'static>(f: F) -> Self
    where
        T: 'static,
    {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }

    /// Return an immediate future, synchronously, which will be
    /// executed asynchronously within the *requester's* context
    /// before the original `.request(...).await` completes.
    pub fn immediate_future<F: Future<Output = T> + 'static>(actor: &'static mut A, f: F) -> Self
    where
        T: 'static,
    {
        Self::ImmediateFuture(actor, Box::new(alloc(f).unwrap()))
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
    fn on_request(&'static mut self, message: M) -> Response<Self, Self::Response>;

    fn respond_with(
        &'static mut self,
        response: Self::Response,
    ) -> (&'static mut Self, Self::Response) {
        (&mut *self, response)
    }
}

/// Return value from a `NotifyHandler` to allow for immediate synchronous handling
/// of the notification or asynchronous handling.
pub enum Completion<A: Actor + 'static> {
    /// See `immediate()`
    Immediate(&'static mut A),

    /// See `defer(future)`
    Defer(Box<dyn Future<Output = &'static mut A>>),
}

impl<A: Actor> Completion<A> {
    /// Indicates the notification has been immediately handled.
    pub fn immediate(actor: &'static mut A) -> Self {
        Self::Immediate(actor)
    }

    /// Provide a future for asynchronous handling of the notification
    /// within this actor's context.
    pub fn defer<F: Future<Output = &'static mut A> + 'static>(f: F) -> Self {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }
}

/// Trait denoting the capability of being notified.
pub trait NotifyHandler<M>
where
    Self: Actor + Sized,
{
    /// Handle the notification.
    fn on_notify(&'static mut self, message: M) -> Completion<Self>;
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
    fn on_event(&'static mut self, event: E) {}
}
