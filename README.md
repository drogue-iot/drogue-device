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
* IoT examples for BLE, Bluetooth Mesh, WiFi and LoRaWAN with OTA functionality.
* Works out of the box with the [Drogue Cloud](https://github.com/drogue-iot/drogue-cloud) connectivity layer.
* Async programming model for writing safe and efficient applications.
* All software is licensed under the Apache 2.0 open source license.

See the [documentation](https://book.drogue.io/drogue-device/dev/index.html) for more information.

Go to our [homepage](https://www.drogue.io) to learn more about the Drogue IoT project.

## Minimum Supported Rust Version

Drogue Device requires the Rust nightly toolchain. If you installed rust using [rustup](https://rustup.rs), all the commands should "just work".

## Hardware

Drogue device runs on any hardware supported by embassy, which at the time of writing includes:

* nRF52 
* STM32
* Raspberry Pi Pico
* Linux, Mac OS X or Windows
* WASM (WebAssembly)

We provide examples for a subset of hardware that we ensure works and that are relevant for IoT.

## Example applications

An overview of the examples can be found in the [documentation](https://book.drogue.io/drogue-device/dev/examples.html).

You can copy the examples if you wish to create an application outside of this repository.

## Developing

To make testing and developing examples a bit easier, we have defined a few commands that you can run from the root folder of the repository that should work with any example. These commands will also ensure that the appropriate bootloader is installed if needed.

To flash an example, run `cargo xtask flash`:

~~~shell
cargo xtask flash examples/nrf52/microbit/ble
~~~

To debug an example, run `cargo xtask debug`:

~~~shell
cargo xtask debug examples/nrf52/microbit/ble
~~~

To just build the example, run `cargo xtask build`:

~~~shell
cargo xtask build examples/nrf52/microbit/ble
~~~

To do a full build of everything including examples:

~~~shell
cargo xtask ci
~~~

### Directory layout

* `boards` - Board Support Package (BSP) for supported boards
* `device` - Library for building IoT applications
* `macros` - macros to load configuration files for the device firmware.
* `bootloader` - Bootloader for all supported boards (required for OTA).
* `examples` - examples that can run on different boards

## Contributing

See the document [CONTRIBUTING.md](CONTRIBUTING.md).

## Community

* [Drogue IoT Matrix Chat Room](https://matrix.to/#/#drogue-iot:matrix.org)
* We have bi-weekly calls at 9:00 AM (GMT). [Check the calendar](https://calendar.google.com/calendar/u/0/embed?src=ofuctjec399jr6kara7n0uidqg@group.calendar.google.com&pli=1) to see which week we are having the next call, and feel free to join!
* [Drogue IoT Forum](https://discourse.drogue.io/)
* [Drogue IoT YouTube channel](https://www.youtube.com/channel/UC7GZUy2hKidvY6V_3QZfCcA)
* [Follow us on Twitter!](https://twitter.com/DrogueIoT)
