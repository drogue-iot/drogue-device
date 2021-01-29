#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;
#[cfg(feature = "stm32l4xx")]
pub mod stm32l4xx;

use crate::domain::time::duration::Milliseconds;

pub trait Timer {
    fn start(&mut self, duration: Milliseconds);
    fn clear_update_interrupt_flag(&mut self);
}
