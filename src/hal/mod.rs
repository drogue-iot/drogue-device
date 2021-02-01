//! General HAL types and traits.

pub mod gpio;
pub mod i2c;
pub mod timer;
pub mod uart;
pub mod spi;

/// Enum for denoting active-high or active-low.
pub enum Active {
    High,
    Low,
}
