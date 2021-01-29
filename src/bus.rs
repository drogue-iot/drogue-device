use crate::device::{Device, DeviceContext, Lifecycle};

use core::cell::UnsafeCell;
use crate::prelude::{Actor, NotificationHandler, Address};
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

pub trait EventConsumer<E> {
    fn on_event(&'static mut self, message: E) {

    }
}

/*
impl<D: Device> NotificationHandler<Lifecycle> for EventBus<D> {
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}
 */


impl<D: Device, M> NotificationHandler<M> for EventBus<D>
    where D: EventConsumer<M>
{
    fn on_notification(&'static mut self, message: M) -> Completion {
        unsafe {
            (&**self.device.get()).on_event(message)
        }
        Completion::immediate()
    }
}

/*
impl<D: Device, M> EventConsumer<M> for EventBus<D>
    where D: EventConsumer<M>
{
    fn on_event(&'static mut self, message: M) {
        unsafe {
            (&**self.device.get()).on_event(message)
        }
    }
}
 */

/*
impl<D: Device, E> EventConsumer<E> for EventBus<D>
    where D: EventConsumer<E>
{
    fn on_event(&'static mut self, message: E) {
        unsafe {
            (&**self.device.get()).on_event(message)
        }
    }
}
 */

impl<D: Device> Address<EventBus<D>> {
    pub fn publish<E: 'static>(&self, message: E)
        where D: EventConsumer<E> + 'static
    {
        self.notify(message)
    }
}
