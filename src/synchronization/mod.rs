//! Synchronization primitive actors.

mod mutex;
mod sempahore;

pub use mutex::{Exclusive, Mutex};

pub use sempahore::{Permit, Semaphore};
