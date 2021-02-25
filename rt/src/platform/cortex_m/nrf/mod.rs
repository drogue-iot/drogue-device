pub mod gpiote;
pub mod timer;

#[cfg(any(
    feature = "chip+nrf52833",
))]
pub mod uarte;
