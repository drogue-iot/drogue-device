use crate::channel::{consts, ArrayLength, Channel, ChannelSend};
use crate::signal::{SignalFuture, SignalSlot};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy::util::DropBomb;

/// Trait that each actor must implement.
pub trait Actor: Sized {
    /// Queue size;
    #[rustfmt::skip]
    type MaxQueueSize<'a>: ArrayLength<ActorMessage<'a, Self>> + ArrayLength<SignalSlot> + 'a where Self: 'a = consts::U1;

    /// The configuration that this actor will expect when mounted.
    type Configuration;

    /// The message type that this actor will handle in `on_message`.
    type Message<'a>: Sized
    where
        Self: 'a;

    /// The future type returned in `on_start`, usually derived from an `async move` block
    /// in the implementation
    type OnStartFuture<'a>: Future<Output = ()>
    where
        Self: 'a;

    /// The future type returned in `on_message`, usually derived from an `async move` block
    /// in the implementation
    type OnMessageFuture<'a>: Future<Output = ()>
    where
        Self: 'a;

    /// Called to mount an actor into the system.
    ///
    /// The actor will be presented with both its own `Address<...>`.
    ///
    /// The default implementation does nothing.
    fn on_mount(&mut self, _: Self::Configuration) {}

    /// Lifecycle event of *start*.
    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_>;

    /// Handle an incoming message for this actor.
    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m>;
}

/// A handle to another actor for dispatching messages.
///
/// Individual actor implementations may augment the `Address` object
/// when appropriate bounds are met to provide method-like invocations.
pub struct Address<'a, A: Actor> {
    state: &'a ActorState<'a, A>,
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(state: &'a ActorState<'a, A>) -> Self {
        Self { state }
    }
}

impl<'a, A: Actor> Address<'a, A> {
    /// Perform an unsafe _async_ message send to the actor behind this address.
    ///
    /// The returned future will be driven to completion by the actor processing the message.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub fn send_ref<'m>(&self, message: &'m mut A::Message<'m>) -> SendFuture<'a, 'm, A>
    where
        'a: 'm,
    {
        self.state.send(message)
    }

    /// Perform an unsafe _async_ message send to the actor behind this address.
    ///
    /// The returned future will be driven to completion by the actor processing the message.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub async fn send<'m>(&self, mut message: A::Message<'m>)
    where
        'a: 'm,
    {
        // Transmute is safe because future is awaited
        self.state
            .send(unsafe { core::mem::transmute(&mut message) })
            .await
    }
}

impl<'a, A: Actor> Copy for Address<'a, A> {}

impl<'a, A: Actor> Clone for Address<'a, A> {
    fn clone(&self) -> Self {
        Self { state: self.state }
    }
}

pub struct ActorState<'a, A: Actor> {
    pub actor: UnsafeCell<A>,
    pub channel: Channel<'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>,
    signals: UnsafeCell<[SignalSlot; 4]>,
}

impl<'a, A: Actor> ActorState<'a, A> {
    pub fn new(actor: A) -> Self {
        let channel: Channel<'a, ActorMessage<A>, A::MaxQueueSize<'a>> = Channel::new();
        Self {
            actor: UnsafeCell::new(actor),
            channel,
            signals: UnsafeCell::new(Default::default()),
        }
    }

    /// Launch the actor main processing loop that never returns.
    pub async fn start(&'a self, _: embassy::executor::Spawner)
    where
        A: Unpin,
    {
        let channel = &self.channel;
        let actor = unsafe { &mut *self.actor.get() };
        core::pin::Pin::new(actor).on_start().await;
        loop {
            let mut message = channel.receive().await;
            let actor = unsafe { &mut *self.actor.get() };
            let m = message.message();
            // Note: we know that the message sender will panic if it doesn't await the completion
            // of the message, thus doing a transmute to pretend that message matches the lifetime
            // of the receiver should be fine...
            let m = unsafe { core::mem::transmute(m) };
            core::pin::Pin::new(actor).on_message(m).await;
        }
    }

    /// Acquire a signal slot if there are any free available
    fn acquire_signal(&self) -> &SignalSlot {
        let signals = unsafe { &mut *self.signals.get() };
        let mut i = 0;
        while i < signals.len() {
            if signals[i].acquire() {
                return &signals[i];
            }
            i += 1;
        }
        panic!("not enough signals!");
    }

    /// Send a message to this actor. The returned future _must_ be awaited before dropped. If it is not
    /// awaited, it will panic.
    fn send<'m>(&'a self, message: &'m mut A::Message<'m>) -> SendFuture<'a, 'm, A>
    where
        'a: 'm,
    {
        let signal = self.acquire_signal();
        let message = unsafe { core::mem::transmute::<_, &'a mut A::Message<'a>>(message) };
        let message = ActorMessage::new(message, signal);
        let chan = self.channel.send(message);
        let sig = SignalFuture::new(signal);
        SendFuture::new(chan, sig)
    }

    /// Mount the underloying actor and initialize the channel.
    pub fn mount(&'a self, config: A::Configuration) -> Address<'a, A> {
        unsafe { &mut *self.actor.get() }.on_mount(config);
        self.channel.initialize();
        Address::new(self)
    }
}

enum SendState {
    WaitChannel,
    WaitSignal,
    Done,
}

pub struct SendFuture<'a, 'm, A: Actor + 'a> {
    channel: ChannelSend<'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>,
    signal: SignalFuture<'a, 'm>,
    state: SendState,
    bomb: Option<DropBomb>,
}

impl<'a, 'm, A: Actor> SendFuture<'a, 'm, A> {
    pub fn new(
        channel: ChannelSend<'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>,
        signal: SignalFuture<'a, 'm>,
    ) -> Self {
        Self {
            channel,
            signal,
            state: SendState::WaitChannel,
            bomb: Some(DropBomb::new()),
        }
    }
}

impl<'a, 'm, A: Actor> Future for SendFuture<'a, 'm, A> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let result = match self.state {
                SendState::WaitChannel => {
                    let result = Pin::new(&mut self.channel).poll(cx);
                    if result.is_ready() {
                        self.state = SendState::WaitSignal;
                    }
                    result
                }
                SendState::WaitSignal => {
                    let result = Pin::new(&mut self.signal).poll(cx);
                    if result.is_ready() {
                        self.state = SendState::Done;
                    }
                    result
                }
                SendState::Done => {
                    self.bomb.take().unwrap().defuse();
                    return Poll::Ready(());
                }
            };
            if result.is_pending() {
                return result;
            }
        }
    }
}

pub struct ActorMessage<'m, A: Actor + 'm> {
    message: *mut A::Message<'m>,
    signal: *const SignalSlot,
}

impl<'m, A: Actor> ActorMessage<'m, A> {
    fn new(message: *mut A::Message<'m>, signal: *const SignalSlot) -> Self {
        Self { message, signal }
    }

    pub fn message(&mut self) -> &mut A::Message<'m> {
        unsafe { &mut *self.message }
    }

    pub fn done(&mut self) {
        unsafe { &*self.signal }.signal();
    }
}

impl<'m, A: Actor> Drop for ActorMessage<'m, A> {
    fn drop(&mut self) {
        self.done();
    }
}

#[cfg(test)]
pub mod testutil {

    //    fn static_actor<A: Actor>(actor: A) -> ActorState<'static, A> {}
}
