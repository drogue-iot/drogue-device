use crate::actor::{Actor, ActorContext};
use crate::sink::{MultiSink, Sink};

use core::cell::{Ref, RefCell, UnsafeCell};
use core::marker::PhantomData;
use heapless::consts::U4;

pub struct Broker<A: Actor> {
    subscribers: UnsafeCell<MultiSink<A::Event, U4>>,
    _marker: PhantomData<A>,
}

impl<A: Actor> Broker<A> {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: UnsafeCell::new(MultiSink::<_, U4>::new()),
            _marker: PhantomData,
        }
    }

    pub fn subscribe(&self, subscriber: &'static dyn Sink<A::Event>) {
        unsafe {
            (&mut *self.subscribers.get()).add(subscriber);
        }
    }

    pub fn publish(&self, message: A::Event) {
        unsafe {
            (&*self.subscribers.get()).send(message);
        }
    }
}
