use crate::actor::{Actor, ActorContext};
use crate::device::{Device, DeviceContext};

use core::cell::UnsafeCell;

pub struct EventBus<D: Device> {
    device: UnsafeCell<*const DeviceContext<D>>,
}

impl<D: Device> Clone for EventBus<D> {
    fn clone(&self) -> Self {
        Self {
            device: unsafe { UnsafeCell::new(*self.device.get()) },
        }
    }
}

impl<D: Device + 'static> EventBus<D> {
    pub fn new(device: &DeviceContext<D>) -> Self {
        Self {
            device: UnsafeCell::new(device),
        }
    }

    pub fn publish<E>(&self, event: E)
    where
        D: EventConsumer<E>,
    {
        unsafe {
            (&**self.device.get()).on_event(event);
        }
    }
}

// pub trait EventProducer<D: Device, M>: Actor<D> {}

pub trait EventConsumer<M> {
    fn on_event(&'static self, message: M)
    where
        Self: Sized,
    {
    }
}
