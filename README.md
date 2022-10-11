# Drogue Device

[![CI Build](https://github.com/drogue-iot/drogue-device/actions/workflows/build.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device/actions/workflows/build.yaml)
[![CI Test](https://github.com/drogue-iot/drogue-device/actions/workflows/test.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device/actions/workflows/test.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)
[![Bors enabled](https://bors.tech/images/badge_small.svg)](https://app.bors.tech/repositories/40676)

Drogue device is a distribution of tools and examples for building embedded IoT applications in Rust.

* Built using [rust](https://www.rust-lang.org), an efficient, memory safe and thread safe programming language.
* Based on [embassy](https://github.com/embassy-rs/embassy), the embedded async project. 
* IoT examples for BLE, BLE Mesh, WiFi and LoRaWAN.
* Async programming model for writing safe and efficient applications.
* All software is licensed under the Apache 2.0 open source license.

See the [documentation](https://book.drogue.io/drogue-device/dev/index.html) for more information and an overview of the examples.

Go to our [homepage](https://www.drogue.io) to learn more about the Drogue IoT project.

## Example application

An overview of the examples can be found in the [documentation](https://book.drogue.io/drogue-device/dev/examples.html).

Drogue device runs on any hardware supported by embassy, which at the time of writing includes:

* nRF52 
* STM32
* Raspberry Pi Pico
* Linux, Mac OS X or Windows
* WASM (WebAssembly)

Once you've found an example you like, you can run `cargo xtask clone <example_dir> <target_dir>` to create a copy with the correct dependencies and project files set up.

### A basic blinky application

~~~rust
#[embassy::main]
async fn main(_spawner: Spawner, p: Peripherals) {
    let mut led = Output::new(p.P0_13, Level::Low, OutputDrive::Standard);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(300)).await;
        led.set_low();
        Timer::after(Duration::from_millis(300)).await;
    }
}
~~~

## Building

To build drogue-device, you must install the [nightly rust toolchain](https://rustup.rs/). Once
installed, you can build and test the framework by running

~~~shell
cargo build
~~~

To do a full build of everything including examples:

~~~shell
cargo xtask ci
~~~

This might require you do install additional toolchains for the examples to build. Recent versions
of cargo should automatically install the toolchain from looking at the `rust-toolchain.toml` file.

To update dependencies, run:

~~~shell
cargo xtask update
~~~

## Directory layout

* `boards` - Board Support Package (BSP) for common boards
* `examples` - examples for different platforms and boards
* `device` - async traits, drivers and actors
  * `device/src/traits` - traits provided by drogue that can be used in async code, such as TCP, WiFi or LoRa
  * `device/src/drivers` - async drivers that implement traits for a one or more peripherals
  * `device/src/network` - network connectivity, common network implementations, HTTP clients,
  * `device/src/actors` - common actors that can be used in applications
* `macros` - macros used by drogue-device and application code


## Contributing

See the document [CONTRIBUTING.md](CONTRIBUTING.md).

## Community

* [Drogue IoT Matrix Chat Room](https://matrix.to/#/#drogue-iot:matrix.org)
* We have bi-weekly calls at 9:00 AM (GMT). [Check the calendar](https://calendar.google.com/calendar/u/0/embed?src=ofuctjec399jr6kara7n0uidqg@group.calendar.google.com&pli=1) to see which week we are having the next call, and feel free to join!
* [Drogue IoT Forum](https://discourse.drogue.io/)
* [Drogue IoT YouTube channel](https://www.youtube.com/channel/UC7GZUy2hKidvY6V_3QZfCcA)
* [Follow us on Twitter!](https://twitter.com/DrogueIoT)
