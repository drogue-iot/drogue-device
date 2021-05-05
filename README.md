# Drogue Device

[![CI](https://github.com/drogue-iot/drogue-device-ng/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device-ng/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

Drogue device is an open source async, no-alloc actor framework for embedded devices, based on [embassy](https://github.com/embassy-rs/embassy). 

* Built using [rust](https://www.rust-lang.org), an efficient, memory safe and thread safe programming language.
* Simplifies writing safe, composable and connected embedded applications.
* Offers built-in drivers for internet connectivity, such as WiFi and LoRaWAN.
* All software is licensed under the Apache 2.0 open source license.

See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.

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
of cargo should automatically install the toolchain from looking at the `rust-toolchain` file.

## Directory layout

* `device` - the source of the drogue-device framework
  * `device/src/kernel` - the actor framework
  * `device/src/traits` - traits provided by drogue that can be used in actors or directly, such as WiFi or LoRa
  * `device/src/drivers` - drivers that implement traits for a one or more peripherals
  * `device/src/actors` - common actors that can be used in applications
* `macros` - macros used by drogue-device and application code
* `examples` - examples for different platforms and boards


## Contributing

See the document [CONTRIBUTING.md](CONTRIBUTING.md).

## Community

* [Drogue IoT Matrix Chat Room](https://matrix.to/#/#drogue-iot:matrix.org)
* We have bi-weekly calls at 9:00 AM (GMT). [Check the calendar](https://calendar.google.com/calendar/u/0/embed?src=ofuctjec399jr6kara7n0uidqg@group.calendar.google.com&pli=1) to see which week we are having the next call, and feel free to join!
* [Drogue IoT Forum](https://discourse.drogue.io/)
* [Drogue IoT YouTube channel](https://www.youtube.com/channel/UC7GZUy2hKidvY6V_3QZfCcA)
* [Follow us on Twitter!](https://twitter.com/DrogueIoT)
