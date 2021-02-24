//! General HAL types and traits.

pub mod gpio;
pub mod timer;
pub mod uart;

/// Enum for denoting active-high or active-low.
pub enum Active {
    High,
    Low,
}
