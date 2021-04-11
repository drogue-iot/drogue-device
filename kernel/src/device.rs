use embassy::executor::Spawner;

pub trait Device {
    fn start(&'static self, spawner: Spawner);
}

pub trait DeviceMounter {
    fn mount(&'static self);
}

pub struct DeviceContext<D: Device + DeviceMounter + 'static> {
    device: &'static D,
}

impl<D: Device + DeviceMounter + 'static> DeviceContext<D> {
    pub fn new(device: &'static D) -> Self {
        Self { device }
    }

    pub fn device(&self) -> &'static D {
        self.device
    }
}
