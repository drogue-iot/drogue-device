use crate::supervisor::Supervisor;

pub trait Device {
    fn start(&'static mut self, supervisor: &mut Supervisor);
}

pub struct DeviceContext<D:Device> {
    device: D,
}

impl<D:Device> DeviceContext<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
        }
    }

    pub fn start(&'static mut self, supervisor: &mut Supervisor) {
        self.device.start( supervisor );
    }
}