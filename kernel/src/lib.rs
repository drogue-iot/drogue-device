#![no_std]

pub mod system;

/// Easy imports for common types and traits.
pub mod prelude {
    pub use crate::system::{
        actor::{Actor, ActorContext, ActorMessage},
        address::Address,
        executor::ActorExecutor,
        signal::SignalSlot,
    };
}
