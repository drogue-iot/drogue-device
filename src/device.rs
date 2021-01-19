use crate::supervisor::Supervisor;

pub trait Device {
    fn start(&'static mut self, supervisor: &mut Supervisor);
}

#[doc(hidden)]
pub struct DeviceContext<D:Device> {
    device: D,
    supervisor: Supervisor,
}

impl<D:Device> DeviceContext<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
            supervisor: Supervisor::new(),
        }
    }

    pub fn start(&'static mut self) -> ! {
        self.device.start( &mut self.supervisor );
        self.supervisor.run_forever()
    }

    pub fn on_interrupt(&'static self, irqn: i16) {
        self.supervisor.on_interrupt(irqn);
    }
}