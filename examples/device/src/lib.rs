#![no_std]
#![feature(type_alias_impl_trait)]

extern crate drogue_device_macros;

pub use drogue_device_macros::entry;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

// Reexports
#[doc(hidden)]
pub mod _export {
    extern crate embassy_executor;
    pub use self::embassy_executor::*;

    #[cfg(feature = "embassy-stm32")]
    extern crate embassy_stm32;

    #[cfg(feature = "embassy-stm32")]
    pub use self::embassy_stm32::*;
}

pub use _export::*;
