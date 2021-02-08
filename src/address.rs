//! Actor addresses

use crate::actor::{Actor, ActorContext};
use crate::bind::Bind;
use crate::handler::{NotifyHandler, RequestHandler};
use core::fmt::Debug;

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

impl<A: Actor + 'static> Address<A> {
    pub(crate) fn new(actor: &'static ActorContext<A>) -> Self {
        Self { actor }
    }

    /// Bind or inject another address into the actor behind this address.
    ///
    /// To accept bound addresses, the target must implement `Bind<...>`
    /// for the appropriate type of address being injected.
    pub fn bind<OA: Actor>(&self, address: Address<OA>)
    where
        A: Bind<OA> + 'static,
        OA: 'static,
    {
        self.actor.bind(address);
    }

    /// Send a non-blocking notification to the actor behind this address.
    ///
    /// To accept the message, the target must implement `NotificationHandler<...>`
    /// for the appropriate type of message being sent.
    pub fn notify<M>(&self, message: M)
    where
        A: NotifyHandler<M>,
        M: 'static,
    {
        self.actor.notify(message)
    }

    /// Perform an _async_ request to the actor behind this address.
    ///
    /// To accept the request and provide a response, the target must implement
    /// `RequestHandler<...>` for the appropriate type of message.
    pub async fn request<M>(&self, message: M) -> <A as RequestHandler<M>>::Response
    where
        A: RequestHandler<M> + 'static,
        M: Debug + 'static,
    {
        self.actor.request(message).await
    }

    /// Perform an unsafe _async_ request to the actor behind this address.
    ///
    /// To accept the request and provide a response, the target must implement
    /// `RequestHandler<...>` for the appropriate type of message.
    ///
    /// # Safety
    ///
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope is unsound.
    pub async fn request_unchecked<M>(&self, message: M) -> <A as RequestHandler<M>>::Response
    where
        A: RequestHandler<M> + 'static,
        M: Debug,
    {
        self.actor.request_unchecked(message).await
    }
}
