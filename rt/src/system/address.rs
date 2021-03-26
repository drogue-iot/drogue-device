//! Actor addresses

use crate::prelude::*;

/// A handle to another actor for dispatching notifications and requests.
///
/// Individual actor implementations may augment the `Address` object
/// when appropriate bounds are met to provide method-like invocations
/// of either non-blocking synchronous `notify(...)` type behaviour or
/// asynchronous `request(...)` type behaviour.
pub struct Address<A: Actor + 'static> {
    actor: &'static ActorContext<A>,
}

impl<A: Actor> Copy for Address<A> {}

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self { actor: self.actor }
    }
}

impl<A> Address<A>
where
    A: Actor + 'static,
{
    pub(crate) fn new(actor: &'static ActorContext<A>) -> Self {
        Self { actor }
    }

    /// Send a non-blocking notification to the actor behind this address.
    ///
    /// To accept the message, the target must implement `NotificationHandler<...>`
    /// for the appropriate type of message being sent.
    pub fn notify(&self, message: A::Request) {
        self.actor.notify(message)
    }

    /// Perform an _async_ request to the actor behind this address.
    ///
    /// To accept the request and provide a response, the target must implement
    /// `Actor<...>` for the appropriate type of message.
    pub async fn request(&self, message: A::Request) -> A::Response {
        self.actor.request(message).await
    }

    /// Perform an unsafe _async_ request to the actor behind this address.
    ///
    /// To accept the request and provide a response, the target must implement
    /// `Actor<...>` for the appropriate type of message.
    ///
    /// # Panics
    ///
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn request_panicking(&self, message: A::Request) -> A::Response {
        self.actor.request_panicking(message).await
    }
}
