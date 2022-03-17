#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::drivers::led::neopixel::{NeoPixel, Rgb8, BLUE, GREEN, RED};
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_nrf::Peripherals;
#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

const STEP_SIZE: f32 = 0.02;

const COLORS: [Rgb8; 5] = [
    RED,
    GREEN,
    BLUE,
    Rgb8::new(0xFF, 0xFF, 0x00), // yellow
    Rgb8::new(0xFF, 0x00, 0xFF), // magenta
];

#[embassy::main]
async fn main(_spawner: Spawner, p: Peripherals) {
    let mut neopixel = defmt::unwrap!(NeoPixel::<'_, _, 1>::new(p.PWM0, p.P0_16));

    let mut dir = 1;
    let mut factor = STEP_SIZE;

    let mut color_index = 0;

    loop {
        let color = COLORS[color_index];
        color_index += 1;
        if color_index >= COLORS.len() {
            color_index = 0;
        }
        loop {
            neopixel.set(&[color.scale(factor)]).await.ok();
            Timer::after(Duration::from_millis(10)).await;

            if dir == 1 {
                factor += STEP_SIZE;
                if factor >= 1.0 {
                    factor -= STEP_SIZE;
                    dir = -1;
                    neopixel.set(&[color.scale(1.0)]).await.ok();
                    Timer::after(Duration::from_millis(500)).await;
                }
            } else {
                factor -= STEP_SIZE;
                if factor <= STEP_SIZE {
                    dir = 1;
                    factor += STEP_SIZE;
                    neopixel.set(&[color.scale(0.0)]).await.ok();
                    Timer::after(Duration::from_millis(200)).await;
                    break;
                }
            }
        }
    }
}
