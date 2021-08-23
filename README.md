# Drogue Device

[![CI](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

Drogue device is an open source async, no-alloc framework for embedded devices. It integrates with [embassy](https://github.com/embassy-rs/embassy), the embedded async project. 

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
* Linux, Mac OS X or Windows

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

    /// The on_mount method is called before any messages are processed for the
    /// Actor. The following parameters are provided:
    /// * The actor configuration
    /// * The address to 'self'
    /// * An inbox from which the actor can await incoming messages
    type OnMountFuture<'a> = impl core::future::Future<Output = ()> + 'a;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm;
    {
        async move {
            loop {
                // Await the next message and invoke the following closure. If async is needed in the processin
                // loop, one can use inbox.next() to retrieve the next item.
                inbox.process(|m| {
                    self.count += 1;
                }).await;
            }
        }
    }
}

/// A struct holding the Actors for the application.
pub struct MyDevice {
    counter: ActorContext<'static, Counter>,
}

/// A static reference to this device holding the device state.
static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

/// The entry point of the application is using the embassy runtime.
#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {

    /// Configuring the device will initialize the global state.
    DEVICE.configure(MyDevice {
        counter: ActorContext::new(Counter{count: 0}),
    });

    /// Mounting the device will spawn embassy tasks for every actor.
    let addr = DEVICE.mount(|device| {
        device.a.mount((), spawner)
    }).await;

    /// The actor address may be used in any embassy task to communicate with the actor.
    addr.request(Increment).unwrap().await;
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
