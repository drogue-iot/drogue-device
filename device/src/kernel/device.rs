use atomic_polyfill::{AtomicU8, Ordering};
use embassy::util::Forever;

const NEW: u8 = 0;
const MOUNTED: u8 = 1;

pub struct DeviceContext<D: 'static> {
    device: Forever<D>,
    state: AtomicU8,
}

impl<D: 'static> DeviceContext<D> {
    pub const fn new() -> Self {
        Self {
            device: Forever::new(),
            state: AtomicU8::new(NEW),
        }
    }

    pub fn mount<F: FnOnce(&'static D) -> R, R>(&'static self, device: D, f: F) -> R {
        match self.state.fetch_add(1, Ordering::Relaxed) {
            NEW => {
                self.device.put(device);
                let device = unsafe { self.device.steal() };
                let r = f(device);
                r
            }
            MOUNTED => {
                panic!("Context already mounted");
            }
            val => {
                panic!("Unexpected state: {}", val);
            }
        }
    }
}

impl<D: 'static> Drop for DeviceContext<D> {
    fn drop(&mut self) {
        match self.state.load(Ordering::Acquire) {
            MOUNTED => {
                panic!("Context must be mounted before it is dropped");
            }
            _ => {}
        }
    }
}
