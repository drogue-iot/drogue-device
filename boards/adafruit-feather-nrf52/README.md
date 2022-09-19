# adafruit-feather-nrf52-bsp

[![crates.io](https://img.shields.io/crates/v/adafruit-feather-nrf52-bsp.svg)](https://crates.io/crates/adafruit-feather-nrf52-bsp)
[![docs.rs](https://docs.rs/adafruit-feather-nrf52-bsp/badge.svg)](https://docs.rs/adafruit-feather-nrf52-bsp)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

adafruit-feather-nrf52-bsp is a board support package (BSP) library for the [Adafruit Feather nRF52](https://learn.adafruit.com/introducing-the-adafruit-nrf52840-feather) type of boards.

## Features

* Uses embassy-nrf HAL for peripherals
* Rust Async/Await


## Example application

```
#![no_std]
#![no_main]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use adafruit_feather_nrf52::*;

use embassy_executor::{executor::Spawner, time::Duration};
use embassy_executor::{executor::Spawner, time::Duration};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = AdafruitFeatherNrf52::default();

    loop {
    	board.blue_led.set_high();
    	Timer::after(Duration::from_millis(500)).await;
    	board.blue_led.set_low();
    	Timer::after(Duration::from_millis(500)).await;
    }
}
```
