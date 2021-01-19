#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod actor;
pub mod address;
#[doc(hidden)]
pub mod alloc;
pub mod device;
pub mod handler;
pub mod interrupt;
pub mod sink;
pub mod supervisor;
#[doc(hidden)]
pub mod macros;

pub mod mutex;

pub mod prelude {
    pub use crate::actor::{
        Actor,
        ActorContext,
    };
    pub use crate::interrupt::{
        Interrupt,
        InterruptContext,
    };
    pub use crate::handler::{
        NotificationHandler,
        RequestHandler,
        Completion,
        Response,
    };
    pub use crate::device::{
        Device,
    };
    pub use crate::address::Address;
    pub use crate::sink::Sink;
    pub use crate::supervisor::Supervisor;
    pub use crate::device;

}

#[cfg(test)]
mod tests;

