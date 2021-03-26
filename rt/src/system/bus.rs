//! Shared device-level event-bus type and trait.

use crate::prelude::*;
use crate::system::device::DeviceContext;

/// The shared device-level event-bus.
///
/// The event-bus is ultimately dispatched through the `Device` implementation
/// of a system using the `EventHandler<...>` trait, which is to be implemented
/// for each expected type of event.
///
/// An `EventBus` may not be directly instantiated, but is created prior to the
/// activation of any actors within the system and may be bound into other
/// actors that wish to `publish` events.
pub struct EventBus<D: Device + 'static> {
    device: &'static DeviceContext<D>,
}

impl<D: Device + 'static> Copy for EventBus<D> {}

impl<D: Device + 'static> Clone for EventBus<D> {
    fn clone(&self) -> Self {
        Self {
            device: self.device,
        }
    }
}

impl<D: Device> EventBus<D> {
    pub(crate) fn new(device: &'static DeviceContext<D>) -> Self {
        Self { device }
    }

    pub fn publish<E: 'static>(&self, message: E)
    where
        D: EventHandler<E> + 'static,
    {
        self.device.on_event(message);
    }
}
