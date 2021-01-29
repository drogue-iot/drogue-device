//! Actor addresses

use crate::actor::{Actor, ActorContext};
use crate::bind::Bind;
use crate::handler::{NotifyHandler, RequestHandler};
use core::cell::UnsafeCell;

/// A handle to another actor for dispatching notifications and requests.
///
/// Individual actor implementations may augment the `Address` object
/// when appropriate bounds are met to provide method-like invocations
/// of either non-blocking synchronous `notify(...)` type behaviour or
/// asynchronous `request(...)` type behaviour.
pub struct Address<A: Actor> {
    actor: UnsafeCell<*const ActorContext<A>>,
}

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self {
            actor: unsafe { UnsafeCell::new(*self.actor.get()) },
        }
    }
}

impl<A: Actor> Address<A> {
    pub(crate) fn new(actor: &ActorContext<A>) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
        }
    }

    /// Bind or inject another address into the actor behind this address.
    ///
    /// To accept bound addresses, the target must implement `Bind<...>`
    /// for the appropriate type of address being injected.
    pub fn bind<OA: Actor>(&self, address: &Address<OA>)
    where
        A: Bind<OA> + 'static,
        OA: 'static,
    {
        unsafe {
            (&**self.actor.get()).bind(address);
        }
    }

    /// Send a non-blocking notification to the actor behind this address.
    ///
    /// To accept the message, the target must implement `NotificationHandler<...>`
    /// for the appropriate type of message being sent.
    pub fn notify<M>(&self, message: M)
    where
        A: NotifyHandler<M> + 'static,
        M: 'static,
    {
        unsafe {
            (&**self.actor.get()).notify(message);
        }
    }

    /// Perform an _async_ request to the actor behind this address.
    ///
    /// To accept the request and provide a response, the target must implement
    /// `RequestHandler<...>` for the appropriate type of message.
    pub async fn request<M>(&self, message: M) -> <A as RequestHandler<M>>::Response
    where
        A: RequestHandler<M> + 'static,
        M: 'static,
    {
        unsafe { (&**self.actor.get()).request(message).await }
    }
}
