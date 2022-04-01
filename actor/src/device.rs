use atomic_polyfill::{AtomicU8, Ordering};
use embassy::util::Forever;

const NEW: u8 = 0;
const CONFIGURED: u8 = 1;

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

    pub fn configure(&'static self, device: D) -> &'static D {
        match self.state.fetch_add(1, Ordering::Relaxed) {
            NEW => self.device.put(device),
            _ => {
                panic!("Context already configured");
            }
        }
    }
}

impl<D: 'static> Drop for DeviceContext<D> {
    fn drop(&mut self) {
        match self.state.load(Ordering::Acquire) {
            CONFIGURED => {
                panic!("Context must be configured before it is dropped");
            }
            _ => {}
        }
    }
}
