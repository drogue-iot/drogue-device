

pub trait Device {
    fn start(&'static self);
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

    pub fn start(&'static self) {
        self.device.start();
    }
}