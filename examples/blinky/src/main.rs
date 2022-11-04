#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

mod board;
use board::*;

pub trait BlinkyBoard {
    type Led: embedded_hal::digital::OutputPin;
    type Button: embedded_hal::digital::InputPin + embedded_hal_async::digital::Wait;

    fn new() -> (Self::Led, Self::Button);
}

#[embassy_executor::main]
async fn main(_s: embassy_executor::Spawner) {
    let (mut led, mut button) = Board::new();
    loop {
        button.wait_for_any_edge().await;
        if button.is_low() {
            led.set_high();
        } else {
            led.set_low();
        }
    }
}
