#[cfg(any(feature = "stm32l4xx"))]
pub mod stm32l4xx;

#[cfg(any(feature = "stm32l1xx"))]
pub mod stm32l1xx;

#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

// ------------------------------------------------------------------------
// ------------------------------------------------------------------------

pub use cortex_m::interrupt::CriticalSection;
pub use cortex_m::interrupt::Mutex;

pub fn with_critical_section<F, R>(f: F) -> R
where
    F: FnOnce(&CriticalSection) -> R,
{
    cortex_m::interrupt::free(f)
}
