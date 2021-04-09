use core::cell::RefCell;
use embassy::executor::{SpawnToken, Spawner};

pub trait Device {
    fn mount(&'static self, spawner: Spawner);
}

pub struct DeviceContext<D: Device + 'static> {
    device: &'static D,
    spawner: RefCell<Option<Spawner>>,
}

impl<D: Device + 'static> DeviceContext<D> {
    pub fn new(device: &'static D) -> Self {
        Self {
            device,
            spawner: RefCell::new(None),
        }
    }

    pub fn device(&self) -> &'static D {
        self.device
    }

    pub fn set_spawner(&self, spawner: Spawner) {
        self.spawner.borrow_mut().replace(spawner);
    }

    pub fn start<F>(&self, token: SpawnToken<F>) {
        self.spawner
            .borrow_mut()
            .as_ref()
            .unwrap()
            .spawn(token)
            .unwrap();
    }
}
