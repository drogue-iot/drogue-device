//! General HAL types and traits.

pub mod arbitrator;
pub mod delayer;
pub mod gpio;
pub mod i2c;
pub mod scheduler;
pub mod spi;
pub mod switchable;
pub mod timer;
pub mod uart;

/// Enum for denoting active-high or active-low.
pub enum Active {
    High,
    Low,
}
