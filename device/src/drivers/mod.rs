#[cfg(feature = "ble")]
pub mod ble;

pub mod button;
pub(crate) mod common;
pub mod dns;
pub mod led;
pub mod lora;
pub mod sensors;

pub trait ActiveLevel {}

/// Discriminator for inputs/outputs that are active on high state.
pub struct ActiveHigh;
impl ActiveLevel for ActiveHigh {}

/// Discriminator for inputs/outputs that are active on low state.
pub struct ActiveLow;
impl ActiveLevel for ActiveLow {}
