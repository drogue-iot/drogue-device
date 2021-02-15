# Drogue device drivers

Drogue-device contains device drivers for different boards and sensors. Drivers follow a common set of patterns that makes it easier to write new drivers. Device drivers can be written in different ways. The common patterns we've seen is:

* *Generic Actor + HAL*: Writing a generic driver that consists of a hardware specific HAL that implements a HAL trait, and an actor that is templatized with the HAL trait as a type parameter. The actor is the API outwards to the embedded application and the drogue-device framework. 

* *Generic trait + Hardware-specific Actor*: Writing a common set of commands that an Actor should support, a trait extends an Actor to handle requests, and writing a hardware-specific driver that implements the trait. This may be easier if a lot of the driver logic is hardware-specific, and there would be little gain in using a common HAL.

In both cases, a `Package` may be used to group multiple actors in a driver together and expose a single primary actor used to interact with the device.

# Generic actor + HAL

## Writing a HAL

A HAL is a trait that supports common functionality that can be implemented for different systems. A HAL can mean different things in different contexts. In the [rust embedded book](https://rust-embedded.github.io/book/design-patterns/hal/index.html), a HAL usually covers a specific family of devices such as STM32 or nRF. In the context of drogue-device, a HAL is similar to how it is used in the [embedded_hal](https://github.com/rust-embedded/embedded-hal) project, and intends to expose that which is necessary for a driver to work on different hardware.

A HAL can look like this:

```rust
pub trait MyHal {
    fn read_data(&self);
}

#[cfg(feature = "myhardware")]
mod myhardware {
    pub struct MyHardware{}

    impl MyHal for MyHardware {
        fn read_data(&self) -> u32;
    }
}
```

The `MyHardware` type is located in a separate module that can be enabled for a particular device.


## Writing the driver logic

A driver is the interface that the drogue-device framework and the embedded application uses. Depending on the use case, there are different ways to structure a driver. A common pattern is to group all driver functionality into a `Package`.

The simplest drivers are those that does not need to pass non-static references in its messages. You can also handle interrupts with an actor driver. This can be achieved by implementing the `Interrupt` trait as well as the actor trait. The common pattern is to use two actors, one for interrupts and one for handling commands. These actors may communicate either through a separate API or using the actor API.

Implementing the Package trait allows a driver to perform initial configuration across multiple sub-components, such as a separate IRQ actor that handles interrupts, and an API actor that handles requests. 

Here is an example driver that uses a HAL to access the hardware, some shared state between the API actor and the Interrupt actor, and the initial configuration.

```rust
pub struct MyDriver<T: MyHal> {
    api: ActorContext<MyComponent>>,
    interrupt: InterruptContext<MyDevice<T>>,
    shared: AtomicU32,
}

pub struct MyComponent {
    shared: Option<&'static AtomicU32>
}

pub struct MyDevice<T: MyHal> {
    hardware: T,
    shared: Option<&'static AtomicU32>
}

impl<T: MyHal> MyDriver<T> {
    fn new<IRQ: Nr>(hardware: T, irq: IRQ) -> Self {
       Self {
           api: ActorContext::new(MyComponent::new()),
           interrupt: InterruptContext::new(new MyDevice(hardware), irq),
           shared: AtomicU32::new(),
       }
    }
}
```

To become a `package`, the driver must implement the `Package` trait. The actors specify the type of configuration they expect, which is passed down from the driver. The package specifies its primary actor which is exposed to the user application.

```rust
impl<T: MyHal> Package for MyDriver<T> {
    type Configuration = ();
    type Primary = MyComponent;
    fn on_mount(&'static self, config: Self::Configuration, supervisor: &mut Supervisor) {
        self.api.mount(&self.shared, supervisor);
        self.support.mount(&self.shared, supervisor)
    }
}

impl Actor for MyComponent {
    type Configuration = &'static AtomicU32;
    fn configure(&mut self, config: Self::Configuration) {
        self.shared.replace(config);
    }
}

impl<T: MyHal> Actor for MyDevice<T> {
    type Configuration = &'static AtomicU32;
    fn configure(&mut self, config: Self::Configuration) {
        self.shared.replace(config);
    }
}
```

The `MyDevice` actor handles interrupts, reads the value of the device and stores it in the shared state:

```rust
impl<T: MyHal> Interrupt for MyDevice<T> {
    fn on_interrupt(&mut self) {
        if let Some(value) = &self.shared {
            value.store(self.hardware.read_value(), Ordering::SeqCst);
        }
    }
}
```

The `MyComponent` actor handles requests:

```rust
// Request types
pub struct ReadValue

impl RequestHandler<ReadValue> for MyComponent {
    type Response = u32;
    
    fn on_request(self, message: ReadValue) -> Response<Self, Self::Response> {
        let value = self.load(Ordering::SeqCst);
        Response::immediate(self, value)
    }
}
```

An example of a generic driver is the [Timer driver](https://github.com/drogue-iot/drogue-device/tree/master/src/driver/timer).

# Generic trait + Hardware-specific actor

In some cases, the driver logic is tightly coupled with the hardware. In that case, it is better to make the driver hardware-specific and expose a common API of commands for using the driver. In that case, the common Request types are defined in the top level module, along with a a trait and an implementation on Address to make it easy to use:

```rust
pub struct ReadValue;

pub trait MyTrait: Actor {
    fn read_value(self) -> Response<Self, u32>;
}

impl<A: MyTrait> RequestHandler<ReadValue> for A {
    type Response = u32;
    fn on_request(self, message: ReadValue) -> Response<Self, Self::Response> {
        self.read_value()
    }
}

impl<A: MyTrait> Address<A> {
    async fn read_value<A: RequestHandler<ReadValue>>(&self) -> u32 {
        self.request(ReadValue).await
    }
}
```

The other parts of the implementation is the same as for a generic driver, except that the `RequestHandler` for `MyComponent` is replaced by the `MyTrait` implementation instead:

```
impl MyTrait for MyComponent {
    fn read_value(self) -> Response<Self, u32> {
        let value = self.load(Ordering::SeqCst);
        Response::immediate(self, value)
    }
}
```

Any actor that implements the `MyTrait` is considered a valid driver from the API POV.

An example of a device specific driver is the [UART driver](https://github.com/drogue-iot/drogue-device/tree/master/src/driver/uart).

# Summary

There are pros and cons of each way to write drivers, but the general advice is to write a genric driver if you can, and fall back to writing a specifid driver if it gets too complicated.
