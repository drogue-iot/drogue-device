//! General HAL types and traits.


pub mod gpio;
pub mod i2c;
pub mod timer;

/// Enum for denoting active-high or active-low.
pub enum Active {
    High,
    Low,
}