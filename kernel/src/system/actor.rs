use crate::system::{
    executor::ActorExecutor,
    signal::{SignalSlot},
};
use core::cell::{RefCell, UnsafeCell};
use core::fmt::Debug;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use core::task::{Context, Poll};
use generic_array::GenericArray;
use heapless::{
    spsc::{Consumer, Producer, Queue},
    ArrayLength,
};

pub struct ActorMessage<A: Actor> {
    pub(crate) signal: RefCell<*const SignalSlot>,
    pub(crate) inner: RefCell<*mut A::Message>,
}

impl<A: Actor> ActorMessage<A> {
    pub fn new(message: &mut A::Message, signal: &SignalSlot) -> Self {
        Self {
            inner: RefCell::new(message),
            signal: RefCell::new(signal),
        }
    }
}

pub trait Actor: Sized {
    type Configuration;
    type Message: Debug;

    fn mount(&mut self, _: Self::Configuration);
    fn poll_message(&mut self, message: &mut Self::Message, cx: &mut Context<'_>) -> Poll<()>;
}

pub trait ActorHandle<A: Actor> {
    fn process_message<'s, 'm>(
        &'s self,
        message: &'m mut A::Message,
    ) -> ActorResponseFuture<'s, 'm>;
}

#[derive(PartialEq)]
pub enum ActorState {
    WAITING = 0,
    READY = 1,
}

impl Into<u8> for ActorState {
    fn into(self) -> u8 {
        self as u8
    }
}

pub struct ActorContext<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>> {
    pub(crate) actor: RefCell<Option<A>>,
    pub(crate) current: RefCell<Option<ActorMessage<A>>>,
    pub(crate) state: AtomicU8,
    pub(crate) in_flight: AtomicBool,

    signals: UnsafeCell<GenericArray<SignalSlot, Q>>,
    messages: UnsafeCell<Queue<ActorMessage<A>, Q>>,

    message_producer: RefCell<Option<Producer<'static, ActorMessage<A>, Q>>>,
    message_consumer: RefCell<Option<Consumer<'static, ActorMessage<A>, Q>>>,
}

impl<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>> ActorContext<A, Q> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: RefCell::new(Some(actor)),
            current: RefCell::new(None),
            state: AtomicU8::new(ActorState::READY.into()),
            in_flight: AtomicBool::new(false),
            signals: UnsafeCell::new(Default::default()),

            messages: UnsafeCell::new(Queue::new()),
            message_producer: RefCell::new(None),
            message_consumer: RefCell::new(None),
        }
    }

    pub fn mount(&'static self, config: A::Configuration, executor: &mut ActorExecutor) {
        executor.activate_actor(self);
        let (mp, mc) = unsafe { (&mut *self.messages.get()).split() };

        self.message_producer.borrow_mut().replace(mp);
        self.message_consumer.borrow_mut().replace(mc);

        self.actor.borrow_mut().as_mut().unwrap().mount(config);
    }

    pub(crate) fn next_message(&self) -> Option<ActorMessage<A>> {
        self.message_consumer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .dequeue()
    }

    fn enqueue_message(&self, message: ActorMessage<A>) {
        self.message_producer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .enqueue(message)
            .unwrap_or_else(|_| panic!("queue full"));
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
}

impl<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>> ActorHandle<A>
    for ActorContext<A, Q>
{
    fn process_message<'s, 'm>(
        &'s self,
        message: &'m mut A::Message,
    ) -> ActorResponseFuture<'s, 'm> {
        let signal = self.acquire_signal();
        let message = ActorMessage::new(message, signal);
        self.enqueue_message(message);
        self.state.store(ActorState::READY.into(), Ordering::SeqCst);
        ActorResponseFuture::new(signal)
    }
}

pub struct ActorResponseFuture<'s, 'm> {
    signal: &'s SignalSlot,
    _marker: PhantomData<&'m ()>,
}

impl<'s> ActorResponseFuture<'s, '_> {
    pub fn new(signal: &'s SignalSlot) -> Self {
        Self {
            signal,
            _marker: PhantomData,
        }
    }
}

impl Future for ActorResponseFuture<'_, '_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.signal.poll_wait(cx)
    }
}
