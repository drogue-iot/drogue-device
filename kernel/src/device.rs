use embassy::{executor::Spawner, util::DropBomb};

pub trait Device {
    fn start(&'static self, spawner: Spawner);
}

pub struct DeviceContext<D: Device + 'static> {
    device: &'static D,
    spawner: Spawner,
    bomb: Option<DropBomb>,
}

impl<D: Device + 'static> DeviceContext<D> {
    pub fn new(spawner: Spawner, device: &'static D) -> Self {
        Self {
            spawner,
            device,
            bomb: Some(DropBomb::new()),
        }
    }

    pub fn device(&self) -> &'static D {
        self.device
    }

    pub fn start(&mut self) {
        self.device.start(self.spawner);
        let _ = self.bomb.take().map(|b| b.defuse());
    }
}
