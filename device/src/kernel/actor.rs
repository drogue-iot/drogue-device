use super::channel::{consts, ArrayLength, Channel, ChannelReceiver, ChannelSend, ChannelSender};
use super::signal::{SignalFuture, SignalSlot};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy::util::DropBomb;
use generic_array::GenericArray;

/// Trait that each actor must implement.
pub trait Actor: Sized {
    /// Queue size;
    #[rustfmt::skip]
    type MaxQueueSize<'a>: ArrayLength<ActorMessage<'a, Self>> + ArrayLength<SignalSlot> + 'a where Self: 'a = consts::U1;

    /// The configuration that this actor will expect when mounted.
    type Configuration = ();

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
    state: &'a ActorContext<'a, A>,
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(state: &'a ActorContext<'a, A>) -> Self {
        Self { state }
    }
}

impl<'a, A: Actor> Address<'a, A> {
    /// Perform an unsafe _async_ message send to the actor behind this address.
    ///
    /// The returned future will be driven to completion by the actor processing the message,
    /// and will complete when the receiving actor have processed the message.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub fn process<'m>(&self, message: &'m mut A::Message<'m>) -> SendFuture<'a, 'm, A>
    where
        'a: 'm,
    {
        self.state.send(message)
    }

    /// Perform an unsafe _async_ message notification to the actor behind this address.
    ///
    /// The returned future will be driven to completion by the actor processing the message,
    /// and will complete when the message have been enqueued, _before_ the message have been
    /// processed.
    ///
    /// # Panics
    /// While the request message may contain non-static references, the user must
    /// ensure that the response to the request is fully `.await`'d before returning.
    /// Leaving an in-flight request dangling while references have gone out of lifetime
    /// scope will result in a panic.
    pub fn notify<'m>(&self, message: A::Message<'a>) -> SendFuture<'a, 'm, A>
    where
        'a: 'm,
    {
        self.state.notify(message)
    }
}

impl<'a, A: Actor> Copy for Address<'a, A> {}

impl<'a, A: Actor> Clone for Address<'a, A> {
    fn clone(&self) -> Self {
        Self { state: self.state }
    }
}

pub struct ActorContext<'a, A: Actor> {
    pub actor: UnsafeCell<A>,
    channel: UnsafeCell<Channel<ActorMessage<'a, A>, A::MaxQueueSize<'a>>>,
    channel_sender: UnsafeCell<Option<ChannelSender<'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>>>,
    channel_receiver:
        UnsafeCell<Option<ChannelReceiver<'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>>>,
    signals: UnsafeCell<GenericArray<SignalSlot, A::MaxQueueSize<'a>>>,
}

impl<'a, A: Actor> ActorContext<'a, A> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            channel: UnsafeCell::new(Channel::new()),
            channel_sender: UnsafeCell::new(None),
            channel_receiver: UnsafeCell::new(None),
            signals: UnsafeCell::new(Default::default()),
        }
    }

    /// Launch the actor main processing loop that never returns.
    pub async fn start(&'a self, _: embassy::executor::Spawner)
    where
        A: Unpin,
    {
        let actor = unsafe { &mut *self.actor.get() };
        let receiver = unsafe { &*self.channel_receiver.get() }.as_ref().unwrap();
        core::pin::Pin::new(actor).on_start().await;
        loop {
            let message = receiver.receive().await;
            let actor = unsafe { &mut *self.actor.get() };
            match message {
                ActorMessage::Send(message, signal) => {
                    core::pin::Pin::new(actor)
                        .on_message(unsafe { &mut *message })
                        .await;
                    unsafe { &*signal }.signal();
                }
                ActorMessage::Notify(mut message) => {
                    // Note: we know that the message sender will panic if it doesn't await the completion
                    // of the message, thus doing a transmute to pretend that message matches the lifetime
                    // of the receiver should be fine...
                    core::pin::Pin::new(actor)
                        .on_message(unsafe { core::mem::transmute(&mut message) })
                        .await;
                }
            }
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
        let message = ActorMessage::new_send(message, signal);
        let chan = {
            let sender = unsafe { &mut *self.channel_sender.get() }.as_mut().unwrap();
            sender.send(message)
        };
        let sig = SignalFuture::new(signal);
        SendFuture::new(chan, Some(sig))
    }

    /// Perform a notification on this actor. The returned future _must_ be awaited before dropped. If it is not
    /// awaited, it will panic.
    fn notify<'m>(&'a self, message: A::Message<'a>) -> SendFuture<'a, 'm, A>
    where
        'a: 'm,
    {
        let message = ActorMessage::new_notify(message);

        let chan = {
            let sender = unsafe { &mut *self.channel_sender.get() }.as_mut().unwrap();
            sender.send(message)
        };
        SendFuture::new(chan, None)
    }

    /// Mount the underloying actor and initialize the channel.
    pub fn mount(&'a self, config: A::Configuration) -> Address<'a, A> {
        unsafe { &mut *self.actor.get() }.on_mount(config);
        let (sender, receiver) = unsafe { &mut *self.channel.get() }.split();
        unsafe { &mut *self.channel_sender.get() }.replace(sender);
        unsafe { &mut *self.channel_receiver.get() }.replace(receiver);
        Address::new(self)
    }
}

#[derive(PartialEq, Eq)]
enum SendState {
    WaitChannel,
    WaitSignal,
    Done,
}

pub struct SendFuture<'a, 'm, A: Actor + 'a> {
    channel: ChannelSend<'m, 'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>,
    signal: Option<SignalFuture<'a, 'm>>,
    state: SendState,
    bomb: Option<DropBomb>,
}

impl<'a, 'm, A: Actor> SendFuture<'a, 'm, A> {
    pub fn new(
        channel: ChannelSend<'m, 'a, ActorMessage<'a, A>, A::MaxQueueSize<'a>>,
        signal: Option<SignalFuture<'a, 'm>>,
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
                        self.state = if self.signal.is_some() {
                            SendState::WaitSignal
                        } else {
                            SendState::Done
                        };
                    }
                    result
                }
                SendState::WaitSignal => {
                    let mut signal = self.signal.as_mut().unwrap();
                    let result = Pin::new(&mut signal).poll(cx);
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

pub enum ActorMessage<'m, A: Actor + 'm> {
    Send(*mut A::Message<'m>, *const SignalSlot),
    Notify(A::Message<'m>),
}

impl<'m, A: Actor> ActorMessage<'m, A> {
    fn new_send(message: *mut A::Message<'m>, signal: *const SignalSlot) -> Self {
        ActorMessage::Send(message, signal)
    }

    fn new_notify(message: A::Message<'m>) -> Self {
        ActorMessage::Notify(message)
    }
}
