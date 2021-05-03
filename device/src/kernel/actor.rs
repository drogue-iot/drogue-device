use super::{
    channel::{Channel, ChannelError, ChannelReceive, ChannelReceiver, ChannelSender},
    signal::{SignalFuture, SignalSlot},
    util::ImmediateFuture,
};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy::util::DropBomb;

/// Trait that each actor must implement.
pub trait Actor: Sized {
    /// The configuration that this actor will expect when mounted.
    type Configuration = ();

    /// The message type that this actor will handle in `on_message`.
    type Message<'a>: Sized
    where
        Self: 'a,
    = ();

    type Response<'a>: Sized + Send
    where
        Self: 'a,
    = ();

    /// Called to mount an actor into the system.
    ///
    /// The actor will be presented with both its own `Address<...>`.
    ///
    /// The default implementation does nothing.
    fn on_mount(&mut self, _: Self::Configuration) {}

    /// The future type returned in `on_start`, usually derived from an `async move` block
    /// in the implementation
    type OnStartFuture<'a>: Future<Output = ()>
    where
        Self: 'a,
    = ImmediateFuture;

    /// Lifecycle event of *start*.
    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_>;

    /// The future type returned in `on_message`, usually derived from an `async move` block
    /// in the implementation
    type OnMessageFuture<'a>: Future<Output = Self::Response<'a>>
    where
        Self: 'a;

    /// Handle an incoming message for this actor.
    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m>;
}

/// A handle to another actor for dispatching messages.
///
/// Individual actor implementations may augment the `Address` object
/// when appropriate bounds are met to provide method-like invocations.
pub struct Address<'a, A: Actor> {
    state: &'a ActorContext<'a, A>,
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(state: &'a ActorContext<'a, A>) -> Self {
        Self { state }
    }
}

impl<'a, A: Actor> Address<'a, A> {
    /// Perform an _async_ message request to the actor behind this address.
    /// If an error occurs when enqueueing the message on the destination actor,
    /// an error is returned.
    ///
    /// The returned future complete when the receiving actor have processed the
    /// message, and the result from processing is made available when the future
    /// is ready.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    #[must_use = "The returned future must be awaited"]
    pub fn request<'m>(&self, message: A::Message<'m>) -> Result<RequestFuture<'a, A>, ActorError>
    where
        'a: 'm,
    {
        self.state.request(message)
    }

    /// Perform an message notification to the actor behind this address. If an error
    /// occurs when enqueueing the message on the destination actor, an error is returned.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the data passed lives as long as the actor.
    pub fn notify<'m>(&self, message: A::Message<'a>) -> Result<(), ActorError> {
        self.state.notify(message)
    }
}

impl<'a, A: Actor> Copy for Address<'a, A> {}

impl<'a, A: Actor> Clone for Address<'a, A> {
    fn clone(&self) -> Self {
        Self { state: self.state }
    }
}

pub struct MessageChannel<'a, T, const N: usize> {
    channel: UnsafeCell<Channel<T, N>>,
    channel_sender: UnsafeCell<Option<ChannelSender<'a, T, N>>>,
    channel_receiver: UnsafeCell<Option<ChannelReceiver<'a, T, N>>>,
}

impl<'a, T, const N: usize> MessageChannel<'a, T, N> {
    pub fn new() -> Self {
        Self {
            channel: UnsafeCell::new(Channel::new()),
            channel_sender: UnsafeCell::new(None),
            channel_receiver: UnsafeCell::new(None),
        }
    }

    pub fn initialize(&'a self) {
        let (sender, receiver) = unsafe { &mut *self.channel.get() }.split();
        unsafe { &mut *self.channel_sender.get() }.replace(sender);
        unsafe { &mut *self.channel_receiver.get() }.replace(receiver);
    }

    pub fn send<'m>(&self, message: T) -> Result<(), ChannelError> {
        let sender = unsafe { &mut *self.channel_sender.get() }.as_mut().unwrap();
        sender.try_send(message)
    }

    pub fn receive<'m>(&self) -> ChannelReceive<'m, 'a, T, N> {
        let receiver = unsafe { &*self.channel_receiver.get() }.as_ref().unwrap();
        receiver.receive()
    }
}

#[derive(Debug)]
pub enum ActorError {
    Channel(ChannelError),
    Signal(SignalError),
}

#[derive(Debug)]
pub enum SignalError {
    NoAvailableSignal,
}

/// A context for an actor, providing signal and message queue. The QLEN parameter
/// is a const generic parameter, and needs to be at least 2 in order for the underlying
/// heapless queue to work. (Due to missing const generic expressions)
#[rustfmt::skip]
pub struct ActorContext<'a, A: Actor, const QLEN: usize= 2>
{
    pub actor: UnsafeCell<A>,
    channel: MessageChannel<'a, ActorMessage<'a, A>, QLEN>,
    // NOTE: This wastes an extra signal because
    signals: UnsafeCell<[SignalSlot<A::Response<'a>>; QLEN]>,
}

impl<'a, A: Actor> ActorContext<'a, A> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            channel: MessageChannel::new(),
            signals: UnsafeCell::new(Default::default()),
        }
    }

    pub(crate) async fn run(&'a self)
    where
        A: Unpin,
    {
        let actor = unsafe { Pin::new_unchecked(&mut *self.actor.get()) };

        actor.on_start().await;

        // crate::log_stack!();
        loop {
            self.process().await;
        }
    }

    pub(crate) async fn process(&'a self) {
        // crate::log_stack!();
        let actor = unsafe { Pin::new_unchecked(&mut *self.actor.get()) };
        match self.channel.receive().await {
            ActorMessage::Request(message, signal) => {
                // crate::log_stack!();
                let value = actor.on_message(message).await;
                unsafe { &*signal }.signal(value);
            }
            ActorMessage::Notify(message) => {
                // crate::log_stack!();
                actor.on_message(message).await;
            }
        }
    }

    /// Launch the actor main processing loop that never returns.
    pub async fn start(&'a self, _: embassy::executor::Spawner)
    where
        A: Unpin,
    {
        self.run().await;
    }

    /// Acquire a signal slot if there are any free available
    fn acquire_signal(&self) -> Result<&SignalSlot<A::Response<'a>>, SignalError> {
        let signals = unsafe { &mut *self.signals.get() };
        let mut i = 0;
        while i < signals.len() {
            if signals[i].acquire() {
                return Ok(&signals[i]);
            }
            i += 1;
        }
        Err(SignalError::NoAvailableSignal)
    }

    /// Perform a request to this actor. The result from processing the request will be provided when the future completes.
    /// The returned future _must_ be awaited before dropped. If it is not
    /// awaited, it will panic.
    fn request<'m>(&'a self, message: A::Message<'m>) -> Result<RequestFuture<'a, A>, ActorError>
    where
        'a: 'm,
    {
        let signal = self.acquire_signal()?;
        // Safety: This is OK because A::Message is Sized.
        let message = unsafe { core::mem::transmute_copy::<_, A::Message<'a>>(&message) };
        let message = ActorMessage::Request(message, signal);
        self.channel.send(message)?;
        let sig = SignalFuture::new(signal);
        Ok(RequestFuture::new(sig))
    }

    /// Perform a notification on this actor. The returned future _must_ be awaited before dropped. If it is not
    /// awaited, it will panic.
    fn notify<'m>(&'a self, message: A::Message<'a>) -> Result<(), ActorError>
    where
        'a: 'm,
    {
        let message = ActorMessage::Notify(message);

        Ok(self.channel.send(message)?)
    }

    /// Mount the underloying actor and initialize the channel.
    pub fn mount(&'a self, config: A::Configuration) -> Address<'a, A> {
        unsafe { &mut *self.actor.get() }.on_mount(config);
        self.channel.initialize();
        Address::new(self)
    }
}
pub struct RequestFuture<'a, A: Actor + 'a> {
    signal: SignalFuture<'a, A::Response<'a>>,
    bomb: Option<DropBomb>,
}

impl<'a, A: Actor> RequestFuture<'a, A> {
    pub fn new(signal: SignalFuture<'a, A::Response<'a>>) -> Self {
        Self {
            signal,
            bomb: Some(DropBomb::new()),
        }
    }
}

impl<'a, A: Actor> Future for RequestFuture<'a, A> {
    type Output = A::Response<'a>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = Pin::new(&mut self.signal).poll(cx);
        if result.is_ready() {
            self.bomb.take().unwrap().defuse();
            self.signal.release();
            return result;
        } else {
            return Poll::Pending;
        }
    }
}

impl From<SignalError> for ActorError {
    fn from(error: SignalError) -> ActorError {
        ActorError::Signal(error)
    }
}

impl From<ChannelError> for ActorError {
    fn from(error: ChannelError) -> ActorError {
        ActorError::Channel(error)
    }
}

pub enum ActorMessage<'m, A: Actor + 'm> {
    Request(A::Message<'m>, *const SignalSlot<A::Response<'m>>),
    Notify(A::Message<'m>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn test_multiple_notifications() {
        let actor = Box::leak(Box::new(ActorContext::new(DummyActor::new())));

        let address = actor.mount(());

        let result_1 = address.notify(TestMessage(0));
        let result_2 = address.notify(TestMessage(1));

        assert!(result_1.is_ok());
        assert!(result_2.is_err());

        step_actor(actor);
        let result_2 = address.notify(TestMessage(1));
        assert!(result_2.is_ok());
    }

    #[test]
    fn test_multiple_requests() {
        let actor = Box::leak(Box::new(ActorContext::new(DummyActor::new())));

        let address = actor.mount(());

        let result_fut_1 = address.request(TestMessage(0));
        let result_fut_2 = address.request(TestMessage(1));
        assert!(result_fut_1.is_ok());
        assert!(result_fut_2.is_err());

        let waker = futures::task::noop_waker_ref();
        let mut cx = std::task::Context::from_waker(waker);

        let mut fut_1 = result_fut_1.unwrap();

        while Pin::new(&mut fut_1).poll(&mut cx).is_pending() {
            step_actor(actor);
        }

        let result_fut_2 = address.request(TestMessage(1));
        assert!(result_fut_2.is_ok());

        let mut fut_2 = result_fut_2.unwrap();
        while Pin::new(&mut fut_2).poll(&mut cx).is_pending() {
            step_actor(actor);
        }
    }
}
