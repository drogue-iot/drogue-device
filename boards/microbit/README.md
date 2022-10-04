# microbit-bsp

[![CI](https://github.com/drogue-iot/microbit-bsp/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/microbit-bsp/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/microbit-bsp.svg)](https://crates.io/crates/microbit-bsp)
[![docs.rs](https://docs.rs/microbit-bsp/badge.svg)](https://docs.rs/microbit-bsp)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

microbit-bsp is a board support package (BSP) library for the BBC micro:bit v2 and newer.

## Features

* LED display driver with fonts
* Uses embassy-nrf HAL for peripherals
* Rust Async/Await


## Example application

```
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use embassy_microbit::*;

use embassy_executor::{executor::Spawner, time::Duration};
use embassy_util::{select, Either};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = Microbit::default();

    let mut display = board.display;
    let mut btn_a = board.btn_a;
    let mut btn_b = board.btn_b;

    display.set_brightness(display::Brightness::MAX);
    display.scroll("Hello, World!").await;
    defmt::info!("Application started, press buttons!");
    loop {
        match select(btn_a.wait_for_low(), btn_b.wait_for_low()).await {
            Either::First(_) => {
                display
                    .display(display::fonts::ARROW_LEFT, Duration::from_secs(1))
                    .await;
            }
            Either::Second(_) => {
                display
                    .display(display::fonts::ARROW_RIGHT, Duration::from_secs(1))
                    .await;
            }
        }
    }
}
```

## Examples

To run an example:

```
cd examples/display
cargo run --release
```
