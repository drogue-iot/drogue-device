// ------------------------------------------------------------------------
// ------------------------------------------------------------------------

pub use cortex_m::interrupt::CriticalSection;
pub use cortex_m::interrupt::Mutex;
pub use cortex_m_rt::exception;

pub fn with_critical_section<F, R>(f: F) -> R
where
    F: FnOnce(&CriticalSection) -> R,
{
    cortex_m::interrupt::free(f)
}

// Copyright James Munns, original code copied from https://github.com/jamesmunns/bbqueue and modified to match
// the atomic types used by drogue-device.
#[cfg(feature = "thumbv6")]
pub mod atomic {
    use core::sync::atomic::{
        AtomicBool, AtomicU8,
        Ordering::{self, Acquire, Release},
    };
    use cortex_m::interrupt::free;

    #[inline(always)]
    pub fn fetch_add(atomic: &AtomicU8, val: u8, _order: Ordering) -> u8 {
        free(|_| {
            let prev = atomic.load(Acquire);
            atomic.store(prev.wrapping_add(val), Release);
            prev
        })
    }

    #[inline(always)]
    pub fn fetch_sub(atomic: &AtomicU8, val: u8, _order: Ordering) -> u8 {
        free(|_| {
            let prev = atomic.load(Acquire);
            atomic.store(prev.wrapping_sub(val), Release);
            prev
        })
    }

    #[inline(always)]
    pub fn swap(atomic: &AtomicBool, val: bool, _order: Ordering) -> bool {
        free(|_| {
            let prev = atomic.load(Acquire);
            atomic.store(val, Release);
            prev
        })
    }
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
