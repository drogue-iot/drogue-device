//! Synchronization primitive actors.

mod mutex;
mod sempahore;
mod signal;

pub use signal::Signal;

pub use mutex::{Exclusive, Lock, Mutex, MutexActor, Unlock};

pub use sempahore::{Permit, SemaphoreActor};
