#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![doc = include_str!("../../README.md")]
pub(crate) mod fmt;

#[cfg(feature = "actors")]
pub mod actors;

pub mod boards;
pub mod device;
pub mod traits;
//
//pub mod drivers;
//
//pub mod domain;
//
//pub mod shared;
//
//
//#[cfg(feature = "dfu")]
//pub mod firmware;
//
//pub mod flash;

#[doc(hidden)]
pub use drogue_device_macros::{self as drogue, config, test as drogue_test};

#[allow(unused_variables)]
pub fn print_stack(file: &'static str, line: u32) {
    let _u: u32 = 1;
    let _uptr: *const u32 = &_u;
    // log::trace!("[{}:{}] SP: 0x{:p}", file, line, &_uptr);
}

#[allow(unused_variables)]
pub fn log_stack(file: &'static str) {
    let _u: u32 = 1;
    let _uptr: *const u32 = &_u;
    //trace!("[{}] SP: 0x{:?}", file, &_uptr);
}

#[allow(unused_variables)]
pub fn print_size<T>(name: &'static str) {
    //log::info!("[{}] size: {}", name, core::mem::size_of::<T>());
}

#[allow(unused_variables)]
pub fn print_value_size<T>(name: &'static str, val: &T) {
    /*    log::info!(
        "[{}] value size: {}",
        name,
        core::mem::size_of_val::<T>(val)
    );*/
}

/// Spawn an actor given a spawner and the actors name, type and instance.
#[macro_export]
macro_rules! spawn_actor {
    ($spawner:ident, $name:ident, $ty:ty, $instance:expr) => {{
        static $name: ::drogue_device::ActorContext<$ty> = ::drogue_device::ActorContext::new();
        $name.mount($spawner, $instance)
    }};
}
