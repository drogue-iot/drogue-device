//! Traits and types for notify, request and event handlers.

use core::future::Future;

use crate::arena::{Arena, Box};
use crate::prelude::Actor;
use crate::system::SystemArena;
use core::mem::transmute;

/// Return value from a `Actor` to allow for synchronous or
/// asynchronous handling of the request.
///
/// *Note:* It is generally better and easier to use the associated
/// functions to construct instances of `Response<T>` than to attempt
/// creating them directly.
pub enum Response<A: Actor + 'static> {
    /// See `immediate(val)`.
    Immediate(A, A::Response),

    /// See `defer(future)`.
    Defer(Box<dyn Future<Output = (A, A::Response)>, SystemArena>),

    /// See `immediate_future(future)`.
    ImmediateFuture(A, Box<dyn Future<Output = A::Response>, SystemArena>),
}

impl<A: Actor + 'static> Response<A> {
    /// Return an immediate value, synchronously, as the response
    /// to the request.
    pub fn immediate(actor: A, val: A::Response) -> Self {
        Self::Immediate(actor, val)
    }

    /// Return a value asynchornously using the supplied future
    /// within the context of *this* actor to calculate the value.
    pub fn defer<F: Future<Output = (A, A::Response)> + 'static>(f: F) -> Self {
        Self::Defer(Box::new(SystemArena::alloc(f).unwrap()))
    }

    /// Return a _non-static_-containing future,
    ///
    /// This is _highly unsafe_.
    ///
    /// # Safety
    ///
    /// This method should only be used if the calling `Actor`
    /// already involves a non-static request message. Non-static `Actor`
    /// will have been invoked using `request_panicking` which will protect against
    /// undefined behaviour by panicking if the caller drops the request future
    /// before completion.
    pub unsafe fn defer_unchecked<F: Future<Output = (A, A::Response)>>(f: F) -> Self {
        let f: &mut dyn Future<Output = (A, A::Response)> = SystemArena::alloc(f).unwrap();
        let f = transmute::<_, &mut (dyn Future<Output = (A, A::Response)> + 'static)>(f);
        Self::Defer(Box::new(f))
    }

    /// Return an immediate future, synchronously, which will be
    /// executed asynchronously within the *requester's* context
    /// before the original `.request(...).await` completes.
    pub fn immediate_future<F: Future<Output = A::Response> + 'static>(actor: A, f: F) -> Self {
        Self::ImmediateFuture(actor, Box::new(SystemArena::alloc(f).unwrap()))
    }
}

/// Return value from an `Actor` to allow for immediate synchronous handling
/// of the notification or asynchronous handling.
pub enum Completion<A: Actor> {
    /// See `immediate()`
    Immediate(A),

    /// See `defer(future)`
    Defer(Box<dyn Future<Output = A>, SystemArena>),
}

impl<A: Actor + 'static> Completion<A> {
    /// Indicates the notification has been immediately handled.
    pub fn immediate(actor: A) -> Self {
        Self::Immediate(actor)
    }

    /// Provide a future for asynchronous handling of the notification
    /// within this actor's context.
    pub fn defer<F: Future<Output = A> + 'static>(f: F) -> Self {
        Self::Defer(Box::new(SystemArena::alloc(f).unwrap()))
    }

    /*
    /// Return a _non-static_-containing future,
    ///
    /// This is _highly unsafe_.
    ///
    /// # Safety
    ///
    /// This method should only be used if the calling `NotifyHandler<>`
    /// already involves a non-static request message. Non-static `RequestHandler<>`
    /// will have been invoked using `notify_panicking` which will protect against
    /// undefined behaviour by panicking if the caller drops the request future
    /// before completion.
    pub unsafe fn defer_unchecked<F: Future<Output = A>>(f: F) -> Self {
        let f: &mut dyn Future<Output = A> = SystemArena::alloc(f).unwrap();
        let f = transmute::<_, &mut (dyn Future<Output = A> + 'static)>(f);
        Self::Defer(Box::new(f))
    }
     */
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
    fn on_event(&'static self, event: E) {}
}
