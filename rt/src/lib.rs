#![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

#[doc(hidden)]
pub mod domain;
pub mod driver;
#[doc(hidden)]
pub mod synchronization;

pub mod api;
mod future;
pub mod hal;
pub mod platform;
pub use drogue_arch as arch;
pub use drogue_arena as arena;
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
        handler::{Completion, EventHandler, RequestHandler, Response},
        interrupt::{Interrupt, InterruptContext},
        package::Package,
        supervisor::Supervisor,
    };

    pub fn log_stack(whr: &'static str) {
        let _u: u32 = 1;
        let _uptr: *const u32 = &_u;
        log::info!("[{}] SP: 0x{:p}", whr, &_uptr);
    }
}
