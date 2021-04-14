#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

pub use drogue_device_kernel::{
    actor::{Actor, ActorState, Address},
    channel::{consts, Channel},
    device::{Device, DeviceContext},
};
pub use drogue_device_macros as drogue;
pub use embassy::*;

#[cfg(feature = "chip+nrf52833")]
pub use embassy_nrf as nrf;

#[doc(hidden)]
pub mod reexport {
    pub use ::embassy;
    #[cfg(feature = "chip+nrf52833")]
    pub use ::embassy_nrf;
    #[cfg(feature = "std")]
    pub use ::embassy_std;
}

#[doc(hidden)]
#[cfg(feature = "std")]
pub use embassy_std::*;
