#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
//! An async, no-alloc actor framework for embedded devices.
//!
//! See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.
//!
//! # Actor System
//!
//! An _actor system_ is a framework that allows for isolating state within narrow contexts, making it easier to reason about system.
//! Within a actor system, the primary component is an _Actor_, which represents the boundary of state usage.
//! Each actor has exclusive access to its own state and only communicates with other actors through message-passing.
//!
//! # Example
//!
//! ```
//! #![macro_use]
//! #![allow(incomplete_features)]
//! #![feature(generic_associated_types)]
//! #![feature(min_type_alias_impl_trait)]
//! #![feature(impl_trait_in_bindings)]
//! #![feature(type_alias_impl_trait)]
//! #![feature(concat_idents)]
//!
//! use drogue_device::*;
//!
//! pub struct MyActor {
//!     name: &'static str,
//! }
//!
//! pub struct SayHello<'m>(&'m str);
//!
//! impl MyActor {
//!     pub fn new(name: &'static str) -> Self {
//!         Self { name }
//!     }
//! }
//!
//! impl Actor for MyActor {
//!     type Configuration = ();
//!     type Message<'a> = SayHello<'a>;
//!     type OnStartFuture<'a> = impl core::future::Future<Output = ()> + 'a;
//!     type OnMessageFuture<'a> = impl core::future::Future<Output = ()> + 'a;
//!
//!     fn on_start(self: core::pin::Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
//!         async move { println!("[{}] started!", self.name) }
//!     }
//!
//!     fn on_message<'m>(
//!         self: core::pin::Pin<&'m mut Self>,
//!         message: &'m mut Self::Message<'m>,
//!     ) -> Self::OnMessageFuture<'m> {
//!         async move {
//!             println!("[{}] Hello {}", self.name, message.0);
//!         }
//!     }
//! }
//!
//! #[derive(drogue::Device)]
//! pub struct MyDevice {
//!     a: ActorState<'static, MyActor>,
//! }
//!
//! #[drogue::configure]
//! fn configure() -> MyDevice {
//!     MyDevice {
//!         a: ActorState::new(MyActor::new("a")),
//!     }
//! }
//!
//! #[drogue::main]
//! async fn main(mut context: DeviceContext<MyDevice>) {
//!     let a_addr = context.device().a.mount(());
//!     context.start();
//!     a_addr.send(&mut SayHello("World")).await;
//! }
//!```
//!
pub use drogue_device_actors as actors;
pub use drogue_device_kernel::{
    actor::{Actor, ActorState, Address},
    channel::{consts, Channel},
    device::{Device, DeviceContext},
    package::Package,
    util::ImmediateFuture,
};

#[doc(hidden)]
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
