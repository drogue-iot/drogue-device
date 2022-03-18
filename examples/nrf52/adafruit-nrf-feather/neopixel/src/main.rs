#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::drivers::led::neopixel::{
    CyclicBrightness, Filter, Gamma, NeoPixel, Rgb8, BLUE, GREEN, RED,
};
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_nrf::Peripherals;
#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

const STEP_SIZE: u8 = 8;

#[embassy::main]
async fn main(_spawner: Spawner, p: Peripherals) {
    let mut neopixel = defmt::unwrap!(NeoPixel::<'_, _, 1>::new(p.PWM0, p.P0_16));

    let cyclic = CyclicBrightness::new(16, 127, STEP_SIZE);
    let mut filter = cyclic.and(Gamma);
    loop {
        neopixel.set_with_filter(&[BLUE], &mut filter).await.ok();
        Timer::after(Duration::from_millis(15)).await;
    }
}
