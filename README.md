# Drogue Device

[![CI](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

Drogue device is an open source async, no-alloc framework for embedded devices, based on [embassy](https://github.com/embassy-rs/embassy). 

* Built using [rust](https://www.rust-lang.org), an efficient, memory safe and thread safe programming language.
* Actor-based programming model for writing safe and composable applications.
* Offers built-in drivers for internet connectivity, such as WiFi and LoRaWAN.
* All software is licensed under the Apache 2.0 open source license.

See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.

## What are Actors?

Actors make it convenient to write stateful concurrent systems using message passing. Actors only process one message at a time, and communicate with other actors by sending messages to their addresses. Actors compose easily due to their decoupled nature, making it easier to maintain an expanding code base.

Actors in drogue-device are *async*, which means that they process messages using async-await support in Rust. This does not mean you have to write async code, but you will have the option to do so. The [async book](https://rust-lang.github.io/async-book/) is a great way to learn more about async Rust.


## Example application

An overview of the examples can be found in [the book](https://book.drogue.io/drogue-device/dev/examples.html) or going to the [examples folder](https://github.com/drogue-iot/drogue-device/tree/main/examples) in this repository.

Drogue device runs on any platform supported by embassy, which at the time of writing includes:

* nRF52
* STM32
* Raspberry Pi Pico
* Unix or Windows

### Example Actor

Following is a simple drogue-device application with a single Actor implementing concurrent access to a counter.

```rust
pub struct Counter {
    count: u32,
}

pub struct Increment;

/// An Actor implements the Actor trait.
impl Actor for Counter {
    type Message<'a> = Increment;

    /// The on_start method is called before any messages are processed for the
    /// Actor, and can be used to write actors that never process messages.
    type OnStartFuture<'a> = impl core::future::Future<Output = ()> + 'a;
    fn on_start(self: core::pin::Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move { }
    }

    /// The on_message method is called for every message that is received
    /// by this actor.
    type OnMessageFuture<'a> = impl core::future::Future<Output = ()> + 'a;
    fn on_message<'m>(
        self: core::pin::Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            self.count += 1;
        }
    }
}

/// A struct holding the Actors for the application.
pub struct Device {
    counter: ActorContext<'static, Counter>,
}

/// The entry point of the application is annotated using the drogue::main macro.
#[drogue::main]
async fn main(context: DeviceContext<Device>) {
    context.configure(Device {
        counter: ActorContext::new(Counter{count: 0}),
    });
    let addr = context.mount(|device, spawner| {
        device.a.mount((), spawner)
    });
    addr.request(Increment).await;
}
```


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
