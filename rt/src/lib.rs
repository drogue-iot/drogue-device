#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

pub extern crate paste;

#[doc(hidden)]
pub mod arena;
pub mod domain;
pub mod driver;
#[doc(hidden)]
pub mod synchronization;

pub mod api;
mod future;
pub mod hal;
pub mod platform;
pub use drogue_arch as arch;
pub mod system;
pub(crate) mod util;

/// Easy imports for common types and traits.
pub mod prelude {
    pub use crate::device;
    //pub use crate::platform::exception;
    pub use crate::system::{
        actor::{Actor, ActorContext, ActorInfo, Configurable},
        address::Address,
        bus::EventBus,
        device::{Device, DeviceConfiguration},
        handler::{Completion, EventHandler, NotifyHandler, RequestHandler, Response},
        interrupt::{Interrupt, InterruptContext},
        package::Package,
        supervisor::Supervisor,
    };
}
