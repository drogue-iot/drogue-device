//! Shared device-level event-bus type and trait.

use crate::prelude::*;
use crate::prelude::device::DeviceContext;

/// The shared device-level event-bus actor.
///
/// The event-bus is ultimately dispatched through the `Device` implementation
/// of a system using the `EventHandler<...>` trait, which is to be implemented
/// for each expected type of event.
///
/// An `EventBus` may not be directly instantiated, but is created prior to the
/// activation of any other actor within the system and may be bound into other
/// actors that wish to `publish` events.
pub struct EventBus<D: Device + 'static> {
    device: &'static DeviceContext<D>,
}

impl<D: Device> EventBus<D> {
    pub(crate) fn new(device: &'static DeviceContext<D>) -> Self {
        Self {
            device,
        }
    }
}

impl<D: Device> Actor for EventBus<D> {}

impl<D: Device, M> NotifyHandler<M> for EventBus<D>
where
    D: EventHandler<M> + 'static,
{
    fn on_notify(self, message: M) -> Completion<Self> {
        self.device.on_event( message );
        Completion::immediate(self)
    }
}

impl<D: Device> Address<EventBus<D>> {
    pub fn publish<E: 'static>(&self, message: E)
    where
        D: EventHandler<E> + 'static,
    {
        self.notify(message)
    }
}
