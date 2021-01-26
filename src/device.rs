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
    fn start(&'static mut self, supervisor: &mut Supervisor);
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

    pub fn start(&'static self) -> ! {
        unsafe {
            (&mut *self.device.get()).start(&mut *self.supervisor.get());
            (&*self.supervisor.get()).run_forever()
        }
    }

    pub fn on_interrupt(&'static self, irqn: i16) {
        unsafe {
            (&*self.supervisor.get()).on_interrupt(irqn);
        }
    }
}
