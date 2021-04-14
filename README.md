# Drogue Device

[![CI](https://github.com/drogue-iot/drogue-device-ng/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device-ng/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

An async, no-alloc actor framework for embedded devices, based on [embassy](https://github.com/embassy-rs).

See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.

## Actor System

An _actor system_ is a framework that allows for isolating state within narrow contexts, making it easier to reason about system.
Within a actor system, the primary component is an _Actor_, which represents the boundary of state usage.
Each actor has exclusive access to its own state and only communicates with other actors through message-passing.

## Async

Each actor is ostensibly single-threaded, able to process a single notification or request at a time, allowing for lock-free processing of each event.
As embedded processors are ostensibly globally single-threaded, supporting multiple actors requires the usage of `async` and `.await` within the Rust ecosystem.
Each actor can therefore process each message either synchronously if its logic is _non-blocking_ or using an `async` block if complex processing is required.

Each event is fully processed, in the order in which it is received, before the next event is considered.

While processing an event, an actor may send a message to another actor, which itself is an asynchronous action, allowing the system to continue to make progress with actors that are able to.

## Messages

All messages are sent using async channels attached to each actor. The channel depth is configurable based on `generic-array` and `heapless`. Once const generics is used by heapless, we will
make the move as well.

## Addresses

Each actor within the system has its own unique `Address` which is used to communicate with the actor (through it's FIFO). 
There is an _async_ `send(msg)` method on each address to send a message asynchronously to the actor, which may only be used from another `async` context, as the sender must `.await` the response.

Specifically, the `Address` for a given actor may expose additional async methods to facility fluent APIs for communicating with the underlying actor.
For instance, the `Address<SimpleLED<...>>` instance has a `turn_on()` and `turn_off()` pair of methods for manipulating the underlying LED.

## State

Each actor is wrapped in a state object which is executed by the embassy runtime. When each state is `mount(...)`ed into the system, its `Address<...>` is made available.

Each Actor in the system defines the configuration it expects to get handed in its `mount()` implementation.

## Bootstrap & Mounting

A top-level `Device` struct maintain members for each actor. The `derive(Device)` type attribute can be used to derive an implementation of `Device` for a particular application.

The device instance is created in a special function marked with `#[drogue::configure]`, and should return a type that derives the `Device` trait.

The application entry point is specified using a function marked with `#[drogue::main]`, and it will be passed a `DeviceContext` that can be used to mount the actors and start the device.

## Packages

In some cases, it may be desirable to have two or more actors involve in a single semantic component or package. Similar to the `Device` trait, the `Package` trait may be derived for any struct in order to bootstrap the actors within it.
