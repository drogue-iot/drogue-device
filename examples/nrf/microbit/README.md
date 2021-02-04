# microbit-uart drogue-device example

This example application runs out of the box on the BBC micro:bit v2.0. It provides a UART echo server that will echo characters you write to the micro:bit serial interface, and it will display ascii characters on the 5x5 LED display.

## Prerequisites

### Hardware

* BBC micro:bit v2.0
* USB to Serial cable
* (Optional) edge connector to access pins - this is just for simplifying connecting to the serial pins

Connect the TX to P15 and RX to P14 on the micro:bit.

![micro:bit edge connector](images/connector.jpg)

### Software

To build and flash the example, you need to have [Rust](https://rustup.rs/), [cargo-embed](https://crates.io/crates/cargo-embed) installed. In pratice you can use whatever tool you want to flash the device, but this guide will assume cargo-embed is used.

## Building

Make sure you have the correct target architecture supported in rust:

```
rustup target add thumbv7em-none-eabihf
```

To build the firmware:

```
cargo build --release
```

## Flashing

Flashing the firmware uses the configuration from the [Embed.toml](Embed.toml) file, which auto-detects the probe connected to your device. If you're experiencing problems, try setting the `usb_vid` and `usb_pid` values to that of your probe (you can find that from lsusb once your board is powered).

The following command will build and flash the firmware and open the debugger console so you can see the console debug output.

```
cargo embed --release
```

## Using minicom program to talk to echo server

Once the firmware is running, you can connect the serial console. I'm using minicom for this, but there are lots of options. The most important thing is to use the following settings:

* 115200 Baudrate
* 8N1 (8 data bits, 1 stop bit)
* No hardware flow control (default is ON for minicom, so remember to turn this off)

The command:

```
minicom -D /dev/ttyUSB0
```

Once connected, press the 'A' button on the micro:bit to start the echo server, and you can start typing characters into the minicom terminal. 

See the [video](https://www.youtube.com/watch?v=wtBmccLh4lw) for a demonstration.
