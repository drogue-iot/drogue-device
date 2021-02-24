extern crate std;

use std::sync::Once;

static INIT: Once = Once::new();
static mut BKL: Option<std::sync::Mutex<()>> = None;

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


#[cfg(not(feature = "thumbv6"))]
pub mod atomic {
    use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    #[inline(always)]
    pub fn fetch_add(atomic: &AtomicU8, val: u8, order: Ordering) -> u8 {
        atomic.fetch_add(val, order)
    }

    #[inline(always)]
    pub fn fetch_sub(atomic: &AtomicU8, val: u8, order: Ordering) -> u8 {
        atomic.fetch_sub(val, order)
    }

    #[inline(always)]
    pub fn swap(atomic: &AtomicBool, val: bool, order: Ordering) -> bool {
        atomic.swap(val, order)
    }
}

