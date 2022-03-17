#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::{Actor, ActorContext, actors, Address, DeviceContext, drivers, Inbox};
use drogue_device::actors::button::{ButtonEvent, ButtonEventHandler};
use drogue_device::actors::led::LedMessage;
use drogue_device::drivers::ActiveLow;
use drogue_device::drivers::led::neopixel::{BLUE, GREEN, NeoPixel, RED, Rgb8};
use drogue_device::traits::button::Event;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_nrf::{gpio::Input, gpio::Output, Peripherals};
use embassy_nrf::config::Config;
use embassy_nrf::gpio::{Level, OutputDrive, Pull};
use embassy_nrf::interrupt::Priority;
use embassy_nrf::peripherals::{P0_11, P0_13, P0_25};
use futures::future::{Either, select};
use futures::pin_mut;
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
async fn main(spawner: Spawner, p: Peripherals) {
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
            neopixel.set(&[color.scale(factor)]).await;
            Timer::after(Duration::from_millis(10)).await;

            if dir == 1 {
                factor += STEP_SIZE;
                if factor >= 1.0 {
                    factor -= STEP_SIZE;
                    dir = -1;
                    neopixel.set(&[color.scale(1.0)]).await;
                    Timer::after(Duration::from_millis(500)).await;
                }
            } else {
                factor -= STEP_SIZE;
                if factor <= STEP_SIZE {
                    dir = 1;
                    factor += STEP_SIZE;
                    neopixel.set(&[color.scale(0.0)]).await;
                    Timer::after(Duration::from_millis(200)).await;
                    break;
                }
            }
        }
    }
}

