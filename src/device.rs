use crate::bus::{EventBus, EventConsumer};
use crate::supervisor::Supervisor;
use core::cell::UnsafeCell;

#[derive(Copy, Clone, Debug)]
pub enum Lifecycle {
    Initialize,
    Start,
    Stop,
    Sleep,
    Hibernate,
}

pub trait Device {
    fn mount(&'static mut self, bus: &EventBus<Self>, supervisor: &mut Supervisor)
    where
        Self: Sized;
}

#[doc(hidden)]
pub struct DeviceContext<D: Device> {
    device: UnsafeCell<D>,
    supervisor: UnsafeCell<Supervisor>,
}

impl<D: Device> DeviceContext<D> {
    pub fn new(device: D) -> Self {
        Self {
            device: UnsafeCell::new(device),
            supervisor: UnsafeCell::new(Supervisor::new()),
        }
    }

    pub fn mount(&'static self) -> ! {
        let bus = EventBus::new(self);
        unsafe {
            (&mut *self.device.get()).mount(&bus, &mut *self.supervisor.get());
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
        D: EventConsumer<E>,
    {
        unsafe {
            (&mut *self.device.get()).on_event(event);
        }
    }
}
