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
pub mod package;
pub mod supervisor;
pub mod synchronization;

pub mod hal;
mod future;

/// Easy imports for common types and traits.
pub mod prelude {
    pub use crate::actor::{Actor, ActorContext, ActorInfo, Configurable};
    pub use crate::address::Address;
    pub use crate::bind::Bind;
    pub use crate::bus::EventBus;
    pub use crate::device;
    pub use crate::device::Device;
    pub use crate::handler::{Completion, EventHandler, NotifyHandler, RequestHandler, Response};
    pub use crate::interrupt::{Interrupt, InterruptContext};
    pub use crate::package::Package;
    pub use crate::supervisor::Supervisor;
}

