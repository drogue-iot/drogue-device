//! Types and traits related to the root-level device and system-wide lifecycle events.

use core::cell::UnsafeCell;

use crate::actor::ActorContext;
use crate::prelude::{Address, EventBus, EventHandler};
use crate::supervisor::Supervisor;

/// System-wide lifecycle events.
///
/// See also `NotificationHandler<...>`.  Each actor within the system is
/// required to implement `NotificationHandler<Lifecycle>` but may opt to
/// ignore any or all of the events.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Lifecycle {
    /// Called after mounting but prior to starting the async executor.
    Initialize,
    /// Called after `Initialize` but prior to starting the async executor.
    Start,
    /// Not currently used.
    Stop,
    /// Not currently used.
    Sleep,
    /// Not currently used.
    Hibernate,
}

/// Trait which must be implemented by all top-level devices which
/// subsequently contain `ActorContext` or `InterruptContext` or other
/// packages.
pub trait Device {
    /// Called when the device is mounted into the system.
    ///
    /// The device *must* propagate the call through to all children `ActorContext`
    /// and `InterruptContext`, either directly or indirectly, in order for them
    /// to be mounted into the system.
    ///
    /// During `mount(...)` the device should perform the appropriate `bind(...)`
    /// for each child in order to inject all required dependencies, including
    /// possible the `EventBus` address which is provided.
    fn mount(&'static mut self, bus_address: &Address<EventBus<Self>>, supervisor: &mut Supervisor)
    where
        Self: Sized;
}

#[doc(hidden)]
pub struct DeviceContext<D: Device> {
    device: UnsafeCell<D>,
    supervisor: UnsafeCell<Supervisor>,
    bus: UnsafeCell<Option<ActorContext<EventBus<D>>>>,
}

impl<D: Device> DeviceContext<D> {
    pub fn new(device: D) -> Self {
        Self {
            device: UnsafeCell::new(device),
            supervisor: UnsafeCell::new(Supervisor::new()),
            bus: UnsafeCell::new(None),
        }
    }

    pub fn mount(&'static self) -> ! {
        let bus = ActorContext::new(EventBus::new(self)).with_name("event-bus");
        unsafe {
            (&mut *self.bus.get()).replace(bus);
            let bus = (&*self.bus.get()).as_ref().unwrap();
            bus.mount(&mut *self.supervisor.get());

            let bus_address = bus.address();
            (&mut *self.device.get()).mount(&bus_address, &mut *self.supervisor.get());
            (&*self.supervisor.get()).run_forever()
        }
    }

    pub fn on_interrupt(&'static self, irqn: i16) {
        unsafe {
            (&*self.supervisor.get()).on_interrupt(irqn);
        }
    }

    pub fn on_event<E>(&'static self, event: E)
    where
        D: EventHandler<E>,
    {
        unsafe {
            (&mut *self.device.get()).on_event(event);
        }
    }
}
