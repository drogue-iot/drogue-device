pub struct Board;

#[cfg_attr(feature = "board+b-l475e-iot01a", path = "b-l475e-iot01a.rs")]
#[cfg_attr(feature = "board+nrf52-dk", path = "nrf52-dk.rs")]
mod _board;
pub use _board::*;
