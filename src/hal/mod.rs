//! General HAL types and traits.

pub mod arbitrator;
pub mod gpio;
pub mod i2c;
pub mod spi;
pub mod timer;
pub mod uart;

/// Enum for denoting active-high or active-low.
pub enum Active {
    High,
    Low,
}
