#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]
#![feature(associated_type_defaults)]
//! Drogue Device is an open source async, no-alloc framework for embedded devices. It integrates with [embassy](https://github.com/embassy-rs/embassy), the embedded async project.
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
//! #![feature(type_alias_impl_trait)]
//! #![feature(const_fn_trait_bound)]
//!
//! # use drogue_device::*;
//!
//! /// A Counter that we wish to create an Actor for.
//! pub struct Counter {
//!     count: u32,
//! }
//!
//! // The message our actor will handle.
//! pub struct Increment;
//!
//! /// An Actor implements the Actor trait.
//! impl Actor for Counter {
//!     /// The Message associated type is the message types that the Actor can receive.
//!     type Message<'a> = Increment;
//!
//!     /// Drogue Device uses a feature from Nightly Rust called Generic Associated Types (GAT) in order
//!     /// to support async functions in traits such as Actor.
//!     type OnMountFuture<'a, M> where M: 'a = impl core::future::Future<Output = ()> + 'a;
//!
//!     /// An actor has to implement the on_mount method. on_mount() is invoked when the internals of an actor is ready,
//!     /// and the actor can begin to receive messages from an inbox.
//!     ///
//!     /// The following arguments are provided:
//!     /// * The address to 'self'
//!     /// * An inbox from which the actor can receive messages
//!     fn on_mount<'m, M>(
//!         &'m mut self,
//!         _: Address<Self>,
//!         inbox: &'m mut M,
//!     ) -> Self::OnMountFuture<'m, M>
//!     where
//!         M: Inbox<Self> + 'm
//!     {
//!         async move {
//!             loop {
//!                 // Await the next message and increment the counter
//!                 if let Some(m) = inbox.next().await {
//!                     self.count += 1;
//!                 }
//!             }
//!         }
//!     }
//! }
//!
//! /// The entry point of the application is using the embassy runtime.
//! #[embassy::main]
//! async fn main(spawner: embassy::executor::Spawner) {
//!
//!     /// Actor state must be static for embassy
//!     static COUNTER: ActorContext<Counter> = ActorContext::new();
//!
//!     // Mounting the Actor will spawn an embassy task
//!     let addr = COUNTER.mount(spawner, Counter {
//!         count: 0
//!     });
//!
//!     // The actor address may be used in any embassy task to communicate with the actor.
//!     addr.request(Increment).unwrap().await;
//! }
//!```
//!

pub(crate) mod fmt;

pub mod kernel;
pub use kernel::{
    actor::{Actor, ActorContext, ActorError, ActorSpawner, Address, Inbox},
    device::DeviceContext,
    package::Package,
    util::ImmediateFuture,
};

pub mod actors;

pub mod traits;

pub mod drivers;

pub mod clients;

pub mod domain;

pub mod bsp;
pub use bsp::Board;

#[cfg(feature = "std")]
pub mod testutil;

#[doc(hidden)]
pub use drogue_device_macros::{self as drogue};

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

#[macro_export]
macro_rules! unborrow {
    ($($name:ident),*) => {
        $(
            #[allow(unused_mut)]
            let mut $name = unsafe { $name.unborrow() };
        )*
    }
}
