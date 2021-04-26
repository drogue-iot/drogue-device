use crate::util::ImmediateFuture;
use core::cell::Cell;
use embassy::executor::Spawner;

pub trait Package {
    fn start(&'static self, spawner: Spawner) -> ImmediateFuture;
}

#[derive(Clone, Copy)]
enum State {
    New,
    Mounted,
}

pub struct PackageContext<P: Package + 'static> {
    package: P,
    state: Cell<State>,
}

impl<P: Package + 'static> PackageContext<P> {
    pub fn new(package: P) -> Self {
        Self {
            package,
            state: Cell::new(State::New),
        }
    }

    pub fn mount<F: FnOnce(&'static P) -> R, R>(&'static self, f: F) -> R {
        match self.state.get() {
            State::New => {
                let r = f(&self.package);
                self.state.set(State::Mounted);
                r
            }
            _ => {
                panic!("Package mount called twice!");
            }
        }
    }

    pub async fn start(&'static self, spawner: Spawner) {
        match self.state.get() {
            State::New => {
                panic!("Package is not mounted!");
            }
            State::Mounted => {
                self.package.start(spawner).await;
            }
        }
    }
}
