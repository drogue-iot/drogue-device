#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod actor;
pub mod address;
#[doc(hidden)]
pub mod alloc;
pub mod bind;
pub mod bus;
pub mod device;
pub mod domain;
pub mod driver;
pub mod handler;
pub mod interrupt;
#[doc(hidden)]
pub mod macros;
pub mod sink;
pub mod supervisor;
pub mod synchronization;

pub mod hal;

pub mod prelude {
    pub use crate::actor::{Actor, ActorContext};
    pub use crate::address::Address;
    pub use crate::bus::{EventBus, EventConsumer};
    pub use crate::device;
    pub use crate::device::{Device, Lifecycle};
    pub use crate::handler::{Completion, NotificationHandler, RequestHandler, Response};
    pub use crate::interrupt::{Interrupt, InterruptContext};
    pub use crate::sink::{MultiSink, Sink};
    pub use crate::supervisor::Supervisor;
}

#[cfg(test)]
mod tests;
