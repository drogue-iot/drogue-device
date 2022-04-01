#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]
#![feature(associated_type_defaults)]
//! Drogue Actor is an open source async, no-alloc framework for embedded devices. It integrates with [embassy](https://github.com/embassy-rs/embassy), the embedded async project.
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
//! #[drogue_device::actor]
//! impl Actor for Counter {
//!     /// The Message associated type is the message types that the Actor can receive.
//!     type Message<'m> = Increment;
//!
//!     /// An actor has to implement the on_mount method. on_mount() is invoked when the internals of an actor is ready,
//!     /// and the actor can begin to receive messages from an inbox.
//!     ///
//!     /// The following arguments are provided:
//!     /// * The address to 'self'
//!     /// * An inbox from which the actor can receive messages
//!     async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, inbox: &mut M)
//!         where M: Inbox<Self> {
//!     {
//!         loop {
//!             // Await the next message and increment the counter
//!             if let Some(m) = inbox.next().await {
//!                 self.count += 1;
//!             }
//!         }
//!     }
//! }
//!
//! /// The entry point of the application is using the embassy runtime.
//! #[embassy::main]
//! async fn main(spawner: embassy::executor::Spawner) {
//!
//!     // Mounting the Actor will spawn an embassy task
//!     let addr = drogue_device::spawn_actor!(spawner, COUNTER, Counter, Counter { count  0 });
//!
//!     // The actor address may be used in any embassy task to communicate with the actor.
//!     let _ = addr.notify(Increment);
//! }
//!```
//!

pub(crate) mod fmt;

mod actor;
mod device;
pub mod signal;

pub use actor::*;
pub use device::*;

#[cfg(feature = "std")]
pub mod testutil;

#[doc(hidden)]
pub use drogue_device_macros::{self as drogue, actor};

/// Spawn an actor given a spawner and the actors name, type and instance.
#[macro_export]
macro_rules! spawn_actor {
    ($spawner:ident, $name:ident, $ty:ty, $instance:expr) => {{
        static $name: ::drogue_device::ActorContext<$ty> = ::drogue_device::ActorContext::new();
        $name.mount($spawner, $instance)
    }};
}
