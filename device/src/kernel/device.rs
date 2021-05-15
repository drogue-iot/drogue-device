use super::actor::ActorSpawner;
use core::cell::Cell;
use embassy::{executor::Spawner, util::Forever};

#[derive(Clone, Copy)]
enum State {
    New,
    Configured,
    Mounted,
}

pub struct DeviceContext<D: 'static> {
    holder: &'static Forever<D>,
    supervisor: ActorSpawner,
    state: Cell<State>,
}

impl<D: 'static> DeviceContext<D> {
    pub fn new(spawner: Spawner, holder: &'static Forever<D>) -> Self {
        Self {
            supervisor: ActorSpawner::new(spawner),
            holder,
            state: Cell::new(State::New),
        }
    }

    pub fn configure(&self, device: D) {
        match self.state.get() {
            State::New => {
                self.holder.put(device);
                self.state.set(State::Configured);
            }
            _ => {
                panic!("Context already configured");
            }
        }
    }

    pub fn mount<F: FnOnce(&'static D, &ActorSpawner) -> R, R>(&self, f: F) -> R {
        match self.state.get() {
            State::Configured => {
                let device = unsafe { self.holder.steal() };
                let r = f(device, &self.supervisor);

                self.state.set(State::Mounted);
                r
            }
            State::New => {
                panic!("Context must be configured before mounted");
            }
            State::Mounted => {
                panic!("Context already mounted");
            }
        }
    }
}

impl<D: 'static> Drop for DeviceContext<D> {
    fn drop(&mut self) {
        match self.state.get() {
            State::Configured => {
                panic!("Context must be mounted before it is dropped");
            }
            State::New => {
                panic!("Context must be configured and mounted before it is dropped");
            }
            _ => {}
        }
    }
}
