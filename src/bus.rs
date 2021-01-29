use crate::device::{Device, DeviceContext};

use core::cell::UnsafeCell;
use crate::prelude::{Actor, NotifyHandler, Address};
use crate::handler::Completion;

pub struct EventBus<D: Device> {
    device: UnsafeCell<*const DeviceContext<D>>,
}

impl<D: Device> EventBus<D> {
    pub(crate) fn new(device: &DeviceContext<D>) -> Self {
        Self {
            device: UnsafeCell::new(device)
        }
    }

}

impl<D: Device> Actor for EventBus<D> {}

pub trait EventHandler<E> {
    fn on_event(&'static mut self, message: E) {

    }
}

impl<D: Device, M> NotifyHandler<M> for EventBus<D>
    where D: EventHandler<M>
{
    fn on_notify(&'static mut self, message: M) -> Completion {
        unsafe {
            (&**self.device.get()).on_event(message)
        }
        Completion::immediate()
    }
}

impl<D: Device> Address<EventBus<D>> {
    pub fn publish<E: 'static>(&self, message: E)
        where D: EventHandler<E> + 'static
    {
        self.notify(message)
    }
}
