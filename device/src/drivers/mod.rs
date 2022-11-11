#[cfg(feature = "ble+softdevice")]
pub mod ble;

pub mod led;

pub mod button;

pub trait ActiveLevel {}

/// Discriminator for inputs/outputs that are active on high state.
pub struct ActiveHigh;
impl ActiveLevel for ActiveHigh {}

/// Discriminator for inputs/outputs that are active on low state.
pub struct ActiveLow;
impl ActiveLevel for ActiveLow {}
