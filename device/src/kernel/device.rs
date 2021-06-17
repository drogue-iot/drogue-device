use atomic_polyfill::{AtomicU8, Ordering};
use core::future::Future;
use embassy::util::Forever;

const NEW: u8 = 0;
const CONFIGURED: u8 = 1;
const MOUNTED: u8 = 2;

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

    pub fn configure(&'static self, device: D) {
        match self.state.fetch_add(1, Ordering::Relaxed) {
            NEW => {
                self.device.put(device);
            }
            _ => {
                panic!("Context already configured");
            }
        }
    }

    pub async fn mount<FUT: Future<Output = R>, F: FnOnce(&'static D) -> FUT, R>(
        &'static self,
        f: F,
    ) -> R {
        match self.state.fetch_add(1, Ordering::Relaxed) {
            CONFIGURED => {
                let device = unsafe { self.device.steal() };
                let r = f(device).await;

                r
            }
            NEW => {
                panic!("Context must be configured before mounted");
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
            CONFIGURED => {
                panic!("Context must be mounted before it is dropped");
            }
            MOUNTED => {
                panic!("Context must be configured and mounted before it is dropped");
            }
            _ => {}
        }
    }
}
