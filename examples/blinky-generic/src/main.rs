#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use drogue_device::{boards::Board, device::Device};

#[embassy_executor::main]
async fn main(_s: embassy_executor::Spawner) {
    let mut device = Board::new();
    let mut led = device.led(0).unwrap();
    let mut button = device.button(0).unwrap();
    loop {
        button.wait_for_any_edge().await;
        if button.is_low() {
            led.set_high();
        } else {
            led.set_low();
        }
    }
}
