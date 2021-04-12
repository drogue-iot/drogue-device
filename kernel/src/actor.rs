use crate::channel::{consts, Channel, ChannelSend};
use crate::signal::{SignalFuture, SignalSlot};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy::util::DropBomb;

pub trait Actor: Unpin {
    type Configuration;
    type Message<'a>: Sized
    where
        Self: 'a;
    type OnStartFuture<'a>: Future<Output = ()>
    where
        Self: 'a;
    type OnMessageFuture<'a>: Future<Output = ()>
    where
        Self: 'a;

    fn on_mount(&mut self, _: Self::Configuration) {}
    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_>;
    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m>;
}

pub struct Address<'a, A: Actor> {
    state: &'a ActorState<'a, A>,
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(state: &'a ActorState<'a, A>) -> Self {
        Self { state }
    }
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn send<'m>(&self, message: &'m A::Message<'m>) -> SendFuture<'a, 'm, A> {
        self.state.send(message)
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
    pub channel: Channel<'a, ActorMessage<'a, A>, consts::U4>,
    signals: UnsafeCell<[SignalSlot; 4]>,
}

impl<'a, A: Actor> ActorState<'a, A> {
    pub fn new(actor: A) -> Self {
        let channel: Channel<'a, ActorMessage<A>, consts::U4> = Channel::new();
        Self {
            actor: UnsafeCell::new(actor),
            channel,
            signals: UnsafeCell::new(Default::default()),
        }
    }

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
    fn send<'m>(&'a self, message: &'m A::Message<'m>) -> SendFuture<'a, 'm, A>
    // impl Future<Output = ()> + 'a
    where
        A: 'm + 'a,
    {
        let signal = self.acquire_signal();
        let message = unsafe { core::mem::transmute::<_, &'a A::Message<'a>>(message) };
        let message = ActorMessage::new(message, signal);
        let chan = self.channel.send(message);
        let sig = SignalFuture::new(signal);
        SendFuture::new(chan, sig)
    }

    pub fn mount(&'a self, config: A::Configuration) -> Address<'a, A> {
        unsafe { &mut *self.actor.get() }.on_mount(config);
        self.channel.initialize();
        Address::new(self)
    }

    pub fn address(&'a self) -> Address<'a, A> {
        Address::new(self)
    }
}

enum SendState {
    WaitChannel,
    WaitSignal,
    Done,
}

pub struct SendFuture<'a, 'm, A: Actor + 'a> {
    channel: ChannelSend<'a, ActorMessage<'a, A>, consts::U4>,
    signal: SignalFuture<'a, 'm>,
    state: SendState,
    bomb: Option<DropBomb>,
}

impl<'a, 'm, A: Actor> SendFuture<'a, 'm, A> {
    pub fn new(
        channel: ChannelSend<'a, ActorMessage<'a, A>, consts::U4>,
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
    message: *const A::Message<'m>,
    signal: *const SignalSlot,
}

impl<'m, A: Actor> ActorMessage<'m, A> {
    fn new(message: *const A::Message<'m>, signal: *const SignalSlot) -> Self {
        Self { message, signal }
    }

    pub fn message(&mut self) -> &A::Message<'m> {
        unsafe { &*self.message }
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
