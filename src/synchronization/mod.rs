//! Synchronization primitive actors.

mod mutex;
mod sempahore;

pub use mutex::{Exclusive, Mutex, Lock, Unlock};

pub use sempahore::{Permit, Semaphore};
