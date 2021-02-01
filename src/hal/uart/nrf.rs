//! Uart implementation for nRF series
#[cfg(feature = "nrf52833")]
use nrf52833_hal as hal;

#[allow(unused_imports)]
#[cfg(any(feature = "nrf52833", feature = "nrf52840"))]
use hal::pac::UARTE1;

#[cfg(feature = "nrf9160")]
use hal::pac::{uarte0_ns as uarte0, UARTE0_NS as UARTE0, UARTE1_NS as UARTE1};

#[cfg(not(feature = "nrf9160"))]
use hal::pac::{uarte0, UARTE0};

use core::ops::Deref;

pub struct Uarte<T>
where
    T: Instance,
{
    uart: T,
}

impl<T> Uarte<T>
where
    T: Instance,
{
    pub fn new(uart: T) -> Self {
        Self { uart }
    }
}

impl<T> crate::hal::uart::Uart for Uarte<T> where T: Instance {}

pub trait Instance: Deref<Target = uarte0::RegisterBlock> + sealed::Sealed {
    fn ptr() -> *const uarte0::RegisterBlock;
}

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for UARTE0 {}
impl Instance for UARTE0 {
    fn ptr() -> *const uarte0::RegisterBlock {
        UARTE0::ptr()
    }
}

#[cfg(any(feature = "52833", feature = "52840", feature = "9160"))]
mod _uarte1 {
    use super::*;
    impl sealed::Sealed for UARTE1 {}
    impl Instance for UARTE1 {
        fn ptr() -> *const uarte0::RegisterBlock {
            UARTE1::ptr()
        }
    }
}
