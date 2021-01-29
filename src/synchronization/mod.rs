//! Synchronization primitive actors.

mod mutex;
mod sempahore;

pub use mutex::{
    Mutex,
    Exclusive,
};

pub use sempahore::{
    Semaphore,
    Permit
};
