# Drogue Device

[![CI](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml/badge.svg)](https://github.com/drogue-iot/drogue-device/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/drogue-device.svg)](https://crates.io/crates/drogue-device)
[![docs.rs](https://docs.rs/drogue-device/badge.svg)](https://docs.rs/drogue-device)
[![Matrix](https://img.shields.io/matrix/drogue-iot:matrix.org)](https://matrix.to/#/#drogue-iot:matrix.org)

An async, no-alloc actor framework for embedded devices.

See [the book](https://book.drogue.io/drogue-device/dev/index.html) for more about the architecture, how to write device drivers, and running some examples.

## Actor System

An _actor system_ is a framework that allows for isolating state within narrow contexts, making it easier to reason about system.
Within a actor system, the primary component is an _Actor_, which represents the boundary of state usage.
Each actor has exclusive access to its own state and only communicates with other actors through message-passing.

The messages being passed can be either _fire-and-forget_ notifications, or _request/response_ interactions.

## Async

Each actor is ostensibly single-threaded, able to process a single notification or request at a time, allowing for lock-free processing of each event.
As embedded processors are ostensibly globally single-threaded, supporting multiple actors requires the usage of `async` and `.await` within the Rust ecosystem.
Each actor can therefore process each event (notification or request/response cycle) either synchronously if its logic is _non-blocking_ or using an `async` block if complex processing is required.

Each event is fully processed, in the order in which it is received, before the next event is considered.
While processing an event, an actor may require a request/response with another actor, which itself is an asynchronous action, allowing the system to continue to make progress with actors that are able to.

## Messages

All messages, both `notify` and `request` style are processed through a single FIFO queue attached to each actor. 
Currently this is set at a depth of 16 items, but with _const generics_ soon coming to stable Rust, the size and overflow behaviour of the FIFOs will be configurable.

## Addresses

Each actor within the system has its own unique `Address` which is used to communicate with the actor (through it's FIFO). 
Most generically, there is a synchronous `notify(msg)` method, which can be called from both synchronous and async contexts, to deliver a message to the actor.
There is also an _async_ `request(msg)->T` method on each address to perform an asynchronous request to the actor, which may only be used from another `async` context, as the requester must `.await` the response.

Specifically, the `Address` for a given actor may expose additional synchronous and async methods to facility fluent APIs for communicating with the underlying actor.
For instance, the `Address<SimpleLED<...>>` instance has a `turn_on()` and `turn_off()` pair of methods for manipulating the underlying LED.

The `Address<Mutex<...>>` instance has an `async lock() -> Exclusive<...>` method for acquiring an exclusive lock to the underlying resource.

## Event Bus

Each system is provided a special actor known as an `EventBus` and its address is made available for publishing events to the root device for further routing.
The `publish(event)` method is available on the `Address<EventBus<...>>` instance.

The root-level `Device` implementation handles the routing and manipulation of events published to the `EventBus` address.

## Contexts

To provide the runtime for actors, and to ensure that no actor directly touches or manipulates another, each actor is wrapped in a _context_, either `ActorContext` or `InterruptContext`.
When each context is `mount(...)`ed into the system, its `Address<...>` is made available.

## Device & Mounting

A top-level `Device` implementation should maintain members for each actor or interrupt within the device.
Upon using the `device!(...)` macro to start the system, it's `mount(...)` will be called where it can then subsequently cause each child actor to be mounted and started.

Each Actor in the system defines the configuration it expects to get handed in its mount() implementation.

## Interrupts

An actor that needs to interact with the hardware interrupts may additionally implement `Interrupt` which provides a hook to be called when the interrupt line is activated.
Since an actor is ostensibly single-threaded, handling an interrupt may only occur when the actor is not otherwise processing messages. 
Likewise, the actor may not process any other messages while handling an interrupt. 

## Packages

In some cases, it may be desirable to have two or more actors involve in a single semantic component or package. 
For instance, one actor may be servicing primarily an interrupt, dispatching messages to another actor which does further work.
By accumulating multiple actors into a reusable `Package`, finer exclusive-locking of resources can be achieved.

One example might be an actor servicing a UART interrupt, sending messages to a non-`Interrupt` actor which processes the inbound byte stream.
