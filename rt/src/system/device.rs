//! Types and traits related to the root-level device and system-wide lifecycle events.

use core::cell::{RefCell, UnsafeCell};

use crate::prelude::*;

pub struct DeviceConfiguration<D>
where
    D: Device + 'static,
{
    pub event_bus: EventBus<D>,
}

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
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor)
    where
        Self: Sized;
}

#[doc(hidden)]
pub struct DeviceContext<D: Device + 'static> {
    device: D,
    supervisor: RefCell<Supervisor>,
    bus: UnsafeCell<Option<EventBus<D>>>,
}

impl<D: Device> DeviceContext<D> {
    pub fn new(device: D) -> Self {
        log::info!("Creating..");
        let d = Self {
            //device: UnsafeCell::new(device),
            device,
            supervisor: RefCell::new(Supervisor::new()),
            bus: UnsafeCell::new(None),
        };
        log::info!("Size of device context is {}", core::mem::size_of_val(&d));
        log::info!("Size of device is {}", core::mem::size_of_val(&d.device));
        log::info!(
            "Size of supervisor is {}",
            core::mem::size_of_val(&d.supervisor)
        );
        log::info!("Size of bus is {}", core::mem::size_of_val(&d.bus));
        d
    }

    pub fn mount(&'static self) {
        unsafe {
            // # Safety
            // UnsafeCell requierd for circular reference between DeviceContext and the EventBus it holds.
            let bus = EventBus::new(self);

            let config = DeviceConfiguration { event_bus: bus.clone() };
            self.device
                .mount(config, &mut *self.supervisor.borrow_mut());
        }
    }

    pub fn run(&'static self) -> ! {
        (&*self.supervisor.borrow()).run_forever()
    }

    pub fn on_interrupt(&'static self, irqn: i16) {
        self.supervisor.borrow().on_interrupt(irqn);
    }

    pub(crate) fn on_event<E>(&'static self, event: E)
    where
        D: EventHandler<E>,
    {
        self.device.on_event(event);
    }
}
