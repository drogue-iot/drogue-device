#[cfg(any(feature = "stm32l4xx", feature = "chip+stm32l4xx"))]
pub mod stm32l4xx;

#[cfg(any(feature = "stm32l1xx"))]
pub mod stm32l1xx;

#[cfg(any(
    feature = "nrf51",
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

