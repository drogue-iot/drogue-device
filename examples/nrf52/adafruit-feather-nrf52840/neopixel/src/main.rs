#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    drivers::led::neopixel::{
        filter::{CyclicBrightness, Filter, Gamma},
        rgbw::{NeoPixelRgbw, BLUE},
    },
};
use adafruit_feather_nrf52::*;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

const STEP_SIZE: u8 = 2;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = AdafruitFeatherNrf52::default();
    let mut neopixel = defmt::unwrap!(NeoPixelRgbw::<'_, _, 1>::new(board.pwm0, board.neopixel));

    let cyclic = CyclicBrightness::new(1, 254, STEP_SIZE);
    let mut filter = cyclic.and(Gamma);
    loop {
        neopixel.set_with_filter(&[BLUE], &mut filter).await.ok();
        Timer::after(Duration::from_millis(20)).await;
    }
}
