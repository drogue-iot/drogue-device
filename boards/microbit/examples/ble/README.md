# microbit-bsp-ble-example

Demonstrating the use of Bluetooth Low Energy (BLE) on the BBC micro:bit.

## Prerequisites

Software:

* [`rustup`](https://rustup.rs/)
* [`probe-run`](https://github.com/knurling-rs/probe-run)
* [`probe-rs-cli`](https://github.com/probe-rs/probe-rs)

Hardware:

* [BBC micro:bit v2](https://microbit.org/)

## Running

Download the [softdevice](https://www.nordicsemi.com/Products/Development-software/S113/Download) and unpack.

Flash the softdevice onto the micro:bit (only needed the first time you run it):

```
probe-rs-cli download s113_nrf52_7.3.0_softdevice.hex --format Hex --chip nRF52833_xxAA
```

Run the application:

```
cargo run --release
```
