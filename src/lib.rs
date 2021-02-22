#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod actor;
pub mod address;
#[doc(hidden)]
pub mod arena;
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

pub mod api;
mod future;
pub mod hal;
pub mod platform;
pub(crate) mod util;

/// Easy imports for common types and traits.
pub mod prelude {
    pub use crate::actor::{Actor, ActorContext, ActorInfo, Configurable};
    pub use crate::address::Address;
    pub use crate::bus::EventBus;
    pub use crate::device;
    pub use crate::device::Device;
    pub use crate::device::DeviceConfiguration;
    pub use crate::handler::{Completion, EventHandler, NotifyHandler, RequestHandler, Response};
    pub use crate::interrupt::{Interrupt, InterruptContext};
    pub use crate::package::Package;
    pub use crate::supervisor::Supervisor;
}
