extern crate std;

use std::sync::Once;

static INIT: Once = Once::new();
static mut BKL: Option<std::sync::Mutex<()>> = None;

pub use cortex_m_rt::exception;

pub struct Mutex<T> {
    val: T,
}

impl<T> Mutex<T> {
    pub fn new(val: T) -> Self {
        Self { val }
    }

    pub fn borrow<'a>(&self, guard: &CriticalSection) -> &T {
        &self.val
    }
}

pub type CriticalSection = std::sync::MutexGuard<'static, ()>;

pub fn with_critical_section<F, R>(f: F) -> R
where
    F: FnOnce(&CriticalSection) -> R,
{
    INIT.call_once(|| unsafe {
        BKL.replace(std::sync::Mutex::new(()));
    });
    let guard = unsafe { BKL.as_ref().unwrap().lock().unwrap() };
    f(&guard)
}
