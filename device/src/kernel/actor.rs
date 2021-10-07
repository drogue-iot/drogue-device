use super::signal::{SignalFuture, SignalSlot};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use embassy::{
    blocking_mutex::kind::Noop,
    channel::mpsc::{self, Channel, Receiver, RecvFuture, Sender},
    executor::{raw::TaskStorage as Task, SpawnError, Spawner},
};
use embassy_hal_common::drop::DropBomb;

type ActorMutex = Noop;

/// Trait that each actor must implement. An Actor must specify a message type
/// it acts on, and an implementation of `on_mount` which is invoked when the
/// actor is started.
///
/// At run time, an Actor is held within an ActorContext, which contains the
/// embassy task and the message queues. The size of the message queue is configured
/// per ActorContext.
pub trait Actor: Sized {
    /// The configuration that this actor will expect when mounted.
    type Configuration = ();

    /// The message type that this actor will receive from its inbox.
    type Message<'a>: Sized
    where
        Self: 'a,
    = ();

    /// The response type that this actor will be expected to respond with for
    /// each message.
    type Response: Sized + Send + Default = ();

    /// The future type returned in `on_mount`, usually derived from an `async move` block
    /// in the implementation using `impl Trait`.
    type OnMountFuture<'m, M>: Future<Output = ()>
    where
        Self: 'm,
        M: 'm;

    /// Called when an actor is mounted (activated). The actor will be provided with its expected
    /// configuration, and address to itself, and an inbox used to receive incoming messages.
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm;
}

pub trait Inbox<'a, A>
where
    A: Actor + 'a,
{
    #[rustfmt::skip]
    type NextFuture<'m>: Future<Output = Option<InboxMessage<'m, A>>> where Self: 'm, 'a: 'm;

    /// Retrieve the next message in the inbox. A default value to use as a response must be
    /// provided to ensure a response is always given.
    ///
    /// This method returns None if the channel is closed.
    #[must_use = "Must set response for message"]
    fn next<'m>(&'m mut self) -> Self::NextFuture<'m>;
}

impl<'a, A: Actor + 'static, const QUEUE_SIZE: usize> Inbox<'a, A>
    for Receiver<'a, ActorMutex, ActorMessage<'a, A>, QUEUE_SIZE>
{
    #[rustfmt::skip]
    type NextFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = Option<InboxMessage<'m, A>>> + 'm;
    fn next<'m>(&'m mut self) -> Self::NextFuture<'m> {
        async move {
            // Safety: This is OK because 'a > 'm and we're doing this to ensure
            // processing loop doesn't abuse the 'fake' lifetime while stored on the queue.
            self.recv().await.map(|m| match m {
                ActorMessage::Request(message, signal) => unsafe {
                    // Ensure we don't run destructor of the copied value
                    let message = core::mem::ManuallyDrop::new(message);
                    InboxMessage::request(
                        core::mem::transmute_copy::<A::Message<'a>, A::Message<'m>>(&message),
                        signal,
                    )
                },
                ActorMessage::Notify(message) => unsafe {
                    // Ensure we don't run destructor of the copied value
                    let message = core::mem::ManuallyDrop::new(message);
                    InboxMessage::notify(
                        core::mem::transmute_copy::<A::Message<'a>, A::Message<'m>>(&message),
                    )
                },
            })
        }
    }
}

/// A handle to another actor for dispatching messages.
///
/// Individual actor implementations may augment the `Address` object
/// when appropriate bounds are met to provide method-like invocations.
pub struct Address<'a, A>
where
    A: Actor + 'static,
{
    state: &'a dyn ActorHandle<'a, A>,
}

pub trait ActorHandle<'a, A>
where
    A: Actor + 'static,
{
    fn request<'m>(&'a self, message: A::Message<'m>) -> Result<RequestFuture<'a, A>, ActorError>
    where
        'a: 'm;
    fn notify<'m>(&'a self, message: A::Message<'a>) -> Result<(), ActorError>
    where
        'a: 'm;
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(state: &'a dyn ActorHandle<'a, A>) -> Self {
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

pub struct MessageChannel<'a, T, const QUEUE_SIZE: usize> {
    channel: UnsafeCell<Channel<ActorMutex, T, QUEUE_SIZE>>,
    channel_sender: UnsafeCell<Option<Sender<'a, ActorMutex, T, QUEUE_SIZE>>>,
    channel_receiver: UnsafeCell<Option<Receiver<'a, ActorMutex, T, QUEUE_SIZE>>>,
}

impl<'a, T, const QUEUE_SIZE: usize> MessageChannel<'a, T, QUEUE_SIZE> {
    pub fn new() -> Self {
        Self {
            channel: UnsafeCell::new(Channel::new()),
            channel_sender: UnsafeCell::new(None),
            channel_receiver: UnsafeCell::new(None),
        }
    }

    pub fn initialize(&'a self) {
        let (sender, receiver) = mpsc::split(unsafe { &mut *self.channel.get() });
        unsafe { &mut *self.channel_sender.get() }.replace(sender);
        unsafe { &mut *self.channel_receiver.get() }.replace(receiver);
    }

    pub fn send<'m>(&self, message: T) -> Result<(), mpsc::TrySendError<T>> {
        let sender = unsafe { &mut *self.channel_sender.get() }.as_mut().unwrap();
        sender.try_send(message)
    }

    pub fn receive(&self) -> RecvFuture<'a, ActorMutex, T, QUEUE_SIZE> {
        let receiver = unsafe { &mut *self.channel_receiver.get() }
            .as_mut()
            .unwrap();
        receiver.recv()
    }

    pub fn inbox(&self) -> &mut Receiver<'a, ActorMutex, T, QUEUE_SIZE> {
        unsafe { &mut *self.channel_receiver.get() }
            .as_mut()
            .unwrap()
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelError {
    Full,
    Closed,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ActorError {
    Channel(ChannelError),
    Signal(SignalError),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SignalError {
    NoAvailableSignal,
}

pub trait ActorSpawner: Clone + Copy {
    fn spawn<F: Future<Output = ()> + 'static>(
        &self,
        task: &'static Task<F>,
        future: F,
    ) -> Result<(), SpawnError>;
}

impl ActorSpawner for Spawner {
    fn spawn<F: Future<Output = ()> + 'static>(
        &self,
        task: &'static Task<F>,
        future: F,
    ) -> Result<(), SpawnError> {
        Spawner::spawn(self, Task::spawn(task, move || future))
    }
}

/// A context for an actor, providing signal and message queue. The QUEUE_SIZE parameter
/// is a const generic parameter, and controls how many messages an Actor can handle.
#[rustfmt::skip]
pub struct ActorContext<'a, A, const QUEUE_SIZE: usize = 1>
where
    A: Actor + 'static,
    [SignalSlot<<A as Actor>::Response>; QUEUE_SIZE]: Default,
{

    task: Task<A::OnMountFuture<'static, Receiver<'static, ActorMutex, ActorMessage<'static, A>, QUEUE_SIZE>>>,
    actor: UnsafeCell<A>,
    channel: MessageChannel<'a, ActorMessage<'a, A>, QUEUE_SIZE>,
    // NOTE: This wastes an extra signal because heapless requires at least 2 slots and
    // const generic expressions doesn't work in this case.
    signals: UnsafeCell<[SignalSlot<A::Response>; QUEUE_SIZE]>,
}

impl<'a, A, const QUEUE_SIZE: usize> ActorHandle<'a, A> for ActorContext<'a, A, QUEUE_SIZE>
where
    A: Actor,
    [SignalSlot<<A as Actor>::Response>; QUEUE_SIZE]: Default,
{
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

        let sent = self.channel.send(message)?;
        Ok(sent)
    }
}

impl<'a, A, const QUEUE_SIZE: usize> ActorContext<'a, A, QUEUE_SIZE>
where
    A: Actor,
    [SignalSlot<<A as Actor>::Response>; QUEUE_SIZE]: Default,
{
    pub fn new(actor: A) -> Self {
        Self {
            task: Task::new(),
            actor: UnsafeCell::new(actor),
            channel: MessageChannel::new(),
            signals: UnsafeCell::new(Default::default()),
        }
    }

    /// Acquire a signal slot if there are any free available
    fn acquire_signal(&self) -> Result<&SignalSlot<A::Response>, SignalError> {
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

    /// Mount the underloying actor and initialize the channel.
    pub fn mount<S: ActorSpawner>(
        &'static self,
        config: A::Configuration,
        spawner: S,
    ) -> Address<'a, A> {
        self.channel.initialize();

        let inbox = self.channel.inbox();
        let address = Address::new(self);
        let future = unsafe { &mut *self.actor.get() }.on_mount(config, address, inbox);
        let task = &self.task;
        // TODO: Map to error?
        spawner.spawn(task, future).unwrap();
        address
    }

    pub(crate) fn initialize(
        &'static self,
        config: A::Configuration,
    ) -> (
        Address<'static, A>,
        A::OnMountFuture<'static, Receiver<'a, ActorMutex, ActorMessage<'a, A>, QUEUE_SIZE>>,
    ) {
        self.channel.initialize();
        let inbox = self.channel.inbox();
        let address = Address::new(self);
        let future = unsafe { &mut *self.actor.get() }.on_mount(config, address, inbox);
        (address, future)
    }
}

pub struct RequestFuture<'a, A: Actor + 'a> {
    signal: SignalFuture<'a, A::Response>,
    bomb: Option<DropBomb>,
}

impl<'a, A: Actor + 'a> RequestFuture<'a, A> {
    pub fn new(signal: SignalFuture<'a, A::Response>) -> Self {
        Self {
            signal,
            bomb: Some(DropBomb::new()),
        }
    }
}

impl<'a, A: Actor + 'a> Future for RequestFuture<'a, A> {
    type Output = A::Response;

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

impl<T> From<mpsc::TrySendError<T>> for ActorError {
    fn from(error: mpsc::TrySendError<T>) -> ActorError {
        ActorError::Channel(match error {
            mpsc::TrySendError::Full(_) => ChannelError::Full,
            mpsc::TrySendError::Closed(_) => ChannelError::Closed,
        })
    }
}

pub(crate) enum ActorMessage<'m, A: Actor + 'm> {
    Request(A::Message<'m>, *const SignalSlot<A::Response>),
    Notify(A::Message<'m>),
}

/// Holds a message retrieved from the Inbox, ensuring that a response
/// is delivered.
pub struct InboxMessage<'m, A>
where
    A: Actor + 'm,
{
    message: A::Message<'m>,
    response: Option<A::Response>,
    signal: Option<*const SignalSlot<A::Response>>,
}

impl<'m, A> InboxMessage<'m, A>
where
    A: Actor + 'm,
{
    pub(crate) fn request(message: A::Message<'m>, signal: *const SignalSlot<A::Response>) -> Self {
        Self {
            message,
            response: Some(Default::default()),
            signal: Some(signal),
        }
    }

    pub(crate) fn notify(message: A::Message<'m>) -> Self {
        Self {
            message,
            response: Some(Default::default()),
            signal: None,
        }
    }

    /// Borrow the message payload.
    pub fn message(&mut self) -> &mut A::Message<'m> {
        &mut self.message
    }

    /// Set a response for this message, which will replace the default response.
    pub fn set_response(&mut self, response: A::Response) {
        self.response.replace(response);
    }
}

impl<'m, A> Drop for InboxMessage<'m, A>
where
    A: Actor + 'm,
{
    fn drop(&mut self) {
        if let Some(signal) = self.signal {
            if let Some(response) = self.response.take() {
                unsafe { &*signal }.signal(response);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn test_multiple_notifications() {
        let actor: &'static mut ActorContext<'static, DummyActor, 1> =
            Box::leak(Box::new(ActorContext::new(DummyActor::new())));

        let (address, mut actor_fut) = actor.initialize(());

        let result_1 = address.notify(TestMessage(0));
        let result_2 = address.notify(TestMessage(1));

        assert!(result_1.is_ok());
        assert!(result_2.is_err());

        step_actor(&mut actor_fut);
        let result_2 = address.notify(TestMessage(1));
        assert!(result_2.is_ok());
    }

    #[test]
    fn test_multiple_requests() {
        let actor: &'static mut ActorContext<'static, DummyActor, 1> =
            Box::leak(Box::new(ActorContext::new(DummyActor::new())));

        let (address, mut actor_fut) = actor.initialize(());

        let result_fut_1 = address.request(TestMessage(0));
        let result_fut_2 = address.request(TestMessage(1));
        assert!(result_fut_1.is_ok());
        assert!(result_fut_2.is_err());

        let waker = futures::task::noop_waker_ref();
        let mut cx = std::task::Context::from_waker(waker);

        let mut fut_1 = result_fut_1.unwrap();

        while Pin::new(&mut fut_1).poll(&mut cx).is_pending() {
            step_actor(&mut actor_fut);
        }

        let result_fut_2 = address.request(TestMessage(1));
        assert!(result_fut_2.is_ok());

        let mut fut_2 = result_fut_2.unwrap();
        while Pin::new(&mut fut_2).poll(&mut cx).is_pending() {
            step_actor(&mut actor_fut);
        }
    }
}
