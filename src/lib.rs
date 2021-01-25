#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod actor;
pub mod address;
#[doc(hidden)]
pub mod alloc;
pub mod bind;
pub mod device;
pub mod handler;
pub mod interrupt;
pub mod sink;
// pub mod broker;
#[doc(hidden)]
pub mod macros;
pub mod supervisor;

pub mod driver;
pub mod synchronization;

pub mod prelude {
    pub use crate::actor::{Actor, ActorContext};
    pub use crate::address::Address;
    pub use crate::device;
    pub use crate::device::Device;
    pub use crate::handler::{Completion, NotificationHandler, RequestHandler, Response};
    pub use crate::interrupt::{Interrupt, InterruptContext};
    pub use crate::sink::{AddSink, Message, MultiSink, Sink};
    pub use crate::supervisor::Supervisor;
}

#[cfg(test)]
mod tests;
