#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]
//! Drogue device is a distribution of libraries and drivers for building embedded applications in Rust.
//!
//! * Built using [rust](https://www.rust-lang.org), an efficient, memory safe and thread safe programming language.
//! * Based on [embassy](https://github.com/embassy-rs/embassy), the embedded async project.
//! * Offers built-in support for IoT with drivers for BLE, BLE Mesh, WiFi and LoRaWAN.
//! * Async programming model for writing safe and efficient applications.
//! * All software is licensed under the Apache 2.0 open source license.
//!
//! See the [documentation](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and for some examples.
//!
//! Go to our [homepage](https://www.drogue.io) to learn more about the Drogue IoT project.
//!
//! ## Example application
//!
//! An overview of the examples can be found in the [documentation](https://book.drogue.io/drogue-device/dev/examples.html).
//!
//! Drogue device runs on any hardware supported by embassy, which at the time of writing includes:
//!
//! * nRF52
//! * STM32
//! * Raspberry Pi Pico
//! * Linux, Mac OS X or Windows
//! * WASM (WebAssembly)
//!
//! Once you've found an example you like, you can run `cargo xtask clone <example_dir> <target_dir>` to create a copy with the correct dependencies and project files set up.
//!
//! ### A basic blinky application
//!
//! ~~~rust
//! #[embassy_executor::main]
//! async fn main(_spawner: Spawner, p: Peripherals) {
//!     let mut led = Output::new(p.P0_13, Level::Low, OutputDrive::Standard);
//!     loop {
//!         led.set_high();
//!         Timer::after(Duration::from_millis(300)).await;
//!         led.set_low();
//!         Timer::after(Duration::from_millis(300)).await;
//!     }
//! }
//! ~~~
pub(crate) mod fmt;

pub mod actors;

pub mod traits;

pub mod drivers;

pub mod domain;

pub mod shared;

#[cfg(feature = "dfu")]
pub mod firmware;

pub mod flash;

pub mod bsp;
pub use bsp::boards;
pub use bsp::Board;

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
