# Drogue device drivers

Drogue-device contains device drivers for different boards and sensors. Drivers follow a common set of patterns that makes it easier to reason about and writing new drivers based on previous examples.

Device drivers normally consists of two parts: a hardware specific HAL that implements a HAL trait, and a driver that is templatized with the HAL trait as a type parameter. The driver is the API outwards to the embedded application and the drogue-device framework.

# Writing a HAL

A HAL is a trait that supports common functionality that can be implemented for different systems. A HAL can mean different things in different contexts. In the [rust embedded book](https://rust-embedded.github.io/book/design-patterns/hal/index.html), a HAL usually covers a specific family of devices such as STM32 or nRF. In the context of drogue-device, a HAL is similar to how it is used in the [embedded_hal](https://github.com/rust-embedded/embedded-hal) project, and intends to expose that which is necessary for a driver to work on different hardware.

A HAL can look like this:

```rust
pub trait MyHal {
    fn talk_to_hardware(&self);
}

#[cfg(feature = "myhardware")]
mod myhardware {
    pub struct MyHardware{}

    impl MyHal for MyHardware {
        fn turn_on(&self) {
            // Set some value in a register
        }
        fn read_data(&self) -> u32;
        fn turn_off(&self) {
            // Set some value in a register
        }
    }
}
```

The `MyHardware` type is located in a separate module that can be enabled for a particular device.


# Writing a driver

A driver is the interface that the drogue-device framework and the embedded application uses. Depending on the use case, there are different types of drivers:

* Actor drivers
* Packaged drivers

Actor drivers are the simplest form of drivers, but requires that you can interact with your device by exchanging messages. For other cases, you can write a packaged driver.

## Actor drivers

Actor drivers are a simple form of drivers that does not need to pass non-static references in its messages. As long as the message types that the driver support can be cloned or copied, this type of driver is the recommended, as it is simpler to write.

The driver implements a NotifyHandler (for fire and forget events) and/or a RequestHandler (for request-response) trait that are used to interact with the driver. The driver is initialized like any other actor.

The driver datatypes wrap an instance of the HAL trait.

```rust
pub struct MyDriver<T: MyHal> {
    hardware: T
}

impl<T: MyHal> MyDriver<T> {
    fn new(hardware: T) -> Self {
       Self {
           hardware
       }
    }
}

```

With the basic datatypes set up, we can implement the Actor trait and some additional traits for handling requests:

```rust
impl<T: MyHal> Actor for MyDriver<T> {}

// Request types
pub struct TurnOn;
pub struct TurnOff;
pub struct ReadData;

impl<T: MyHal> NotifyHandler<TurnOn> for MyDriver<T> {
    fn on_notify(&'static mut self, message: TurnOn) -> Completion<Self> {
        self.hardware.turn_on();
        Completion::immediate(self)
    }
}

impl<T: MyHal> NotifyHandler<TurnOff> for MyDriver<T> {
    fn on_notify(&'static mut self, message: TurnOff) -> Completion<Self> {
        self.hardware.turn_off();
        Completion::immediate(self)
    }
}

impl<T: MyHal> RequestHandler<ReadData> for MyDriver<T> {
    type Response = u32;
    
    fn on_request(&'static mut self, message: ReadData) -> Response<Self, Self::Response> {
        let value = self.hardware.read_data();
        Response::immediate(self, value)
    }
}
```

And with that, you get an Actor capable of interacting with different but similar hardware.

Application code can interact with the driver by sending requests to the actor:

```rust
driver.notify(TurnOn);
let value = driver.request(ReadValue).await;
```

NOTE: You can also handle interrupts with an actor driver. This can be achieved by implementing the `Interrupt` trait as well as the actor trait.

A complete example of an actor driver with interrupts is the [Timer driver](https://github.com/drogue-iot/drogue-device/tree/master/src/driver/timer).

## Packaged drivers

Packaged drivers implement the Package trait of drogue-device, and are useful when a driver encapsulates multiple underlying components, potentially from different places. Implementing the Package trait allows a driver to perform initial configuration across multiple sub-components, such as a separate IRQ actor that handles interrupts, and an API actor that handles requests. 

This type of driver is also useful if for some reason your actor data cannot be handled by the actor framework. A packaged driver can wrap its API actor in a Mutex actor in order to allow users of the driver to pass any kind of data that doesn't have to meet the constraint of the actor framework or if you want to expose the API of an existing component without implementing an Actor.

This example packaged driver uses the same HAL as in for the actor driver, but uses some shared state that sub-actors write and read.

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

To become a `package`, the driver must implement the `Package` trait, and the sub-components must implement the `Configurable` trait in order to get the configuration applied:

```rust
impl<D: Device, T: MyHal> Package<D, Mutex<MyComponent>> for MyDriver<T> {
    fn mount(&'static self, _: &Address<EventBus<D>>, supervisor: &mut Supervisor) {
        self.api.mount(supervisor);
        self.support.mount(supervisor)
        
        self.api.configure(&self.shared);
        self.support.configure(&self.shared);
    }
}

impl Configurable for MyDevice<T> {
    type Configuration = AtomicU32;
    fn configure(&mut self, config: &'static Self::Configuration) {
        self.shared.replace(config);
    }
}

impl<T: MyHal> Configurable for MyDevice<T> {
    type Configuration = AtomicU32;
    fn configure(&mut self, config: &'static Self::Configuration) {
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
impl Actor for MyComponent {}

// Request types
pub struct ReadValue

impl RequestHandler<ReadValue> for MyComponent {
    type Response = u32;
    
    fn on_request(&'static mut self, message: ReadValue) -> Response<Self, Self::Response> {
        let value = self..load(Ordering::SeqCst);
        Response::immediate(self, value)
    }
}
```

An example of a packaged driver is the [UART driver](https://github.com/drogue-iot/drogue-device/tree/master/src/driver/uart).

# Summary

There are pros and cons of each way to write drivers, but the general advice is to write an actor based driver if you can, and fall back to writing a packaged driver if you cannot live with the constraints.

