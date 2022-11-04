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
* IoT examples for BLE, BLE Mesh, WiFi and LoRaWAN that can run on multiple boards.
* Async programming model for writing safe and efficient applications.
* All software is licensed under the Apache 2.0 open source license.

See the [documentation](https://book.drogue.io/drogue-device/dev/index.html) for more information and an overview of the examples.

Go to our [homepage](https://www.drogue.io) to learn more about the Drogue IoT project.

## Minimum Supported Rust Version

Drogue Device requires the Rust nightly toolchain. If you installed rust using [rustup](rustup.rs), all the commands should "just work".

## Example applications

An overview of the examples can be found in the [documentation](https://book.drogue.io/drogue-device/dev/examples.html).

Drogue device runs on any hardware supported by embassy, which at the time of writing includes:

* nRF52 
* STM32
* Raspberry Pi Pico
* Linux, Mac OS X or Windows
* WASM (WebAssembly)

You can copy the examples if you wish to create an application outside of this repository. Remember to update the corresponding dependencies to use versions from git or crates.io.

## Flashing examples

To flash an example, connect one of the [supported boards](), and run:

~~~shell
cargo xtask flash nrf52-dk examples/blinky
~~~

To debug an example, run `cargo xtask debug`:

~~~shell
cargo xtask debug nrf52-dk examples/blinky
~~~

To just build the example, run `cargo xtask build`:

~~~shell
cargo xtask build nrf52-dk examples/blinky
~~~

## Developing

To test the tools themselves:

~~~shell
cargo test
~~~

To do a full build of everything including examples:

~~~shell
cargo xtask ci
~~~

### Directory layout

* `boards` - Board Support Package (BSP) and memory layout for supported boards
* `examples` - examples that can run on different boards
* `device` - Library for building IoT ready applications
  * `device/src/http` - Client for using Drogue Cloud using HTTP.
  * `device/src/mqtt` - Client for using Drogue Cloud using MQTT.
  * `device/src/ota` - Over The Air firmware updates with Drogue Cloud.
* `bootloader` - Bootloader for all supported boards (required for OTA).
* `macros` - macros to load configuration files for the device firmware.


## Contributing

See the document [CONTRIBUTING.md](CONTRIBUTING.md).

## Community

* [Drogue IoT Matrix Chat Room](https://matrix.to/#/#drogue-iot:matrix.org)
* We have bi-weekly calls at 9:00 AM (GMT). [Check the calendar](https://calendar.google.com/calendar/u/0/embed?src=ofuctjec399jr6kara7n0uidqg@group.calendar.google.com&pli=1) to see which week we are having the next call, and feel free to join!
* [Drogue IoT Forum](https://discourse.drogue.io/)
* [Drogue IoT YouTube channel](https://www.youtube.com/channel/UC7GZUy2hKidvY6V_3QZfCcA)
* [Follow us on Twitter!](https://twitter.com/DrogueIoT)
