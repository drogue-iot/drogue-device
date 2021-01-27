use crate::actor::{Actor, ActorContext};
use crate::device::{Device, DeviceContext};

use core::cell::UnsafeCell;

pub struct EventBus<D: Device> {
    device: UnsafeCell<*const D>,
}

impl<D: Device> EventBus<D> {
    pub fn new(device: &'static D) -> Self {
        Self {
            device: UnsafeCell::new(device),
        }
    }

    pub fn publish<P, E>(&self, event: E)
    where
        D: EventConsumer<E>,
        P: EventProducer<D, E>,
    {
        unsafe {
            (&**self.device.get()).on_event(event);
        }
    }
}

pub trait EventProducer<D: Device, M>: Actor<D> {}

pub trait EventConsumer<M> {
    fn on_event(&self, message: M)
    where
        Self: Sized,
    {
    }
}
