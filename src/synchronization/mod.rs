//! Synchronization primitive actors.

mod sempahore;
mod signal;

pub use signal::Signal;

pub use sempahore::{Permit, SemaphoreActor};
