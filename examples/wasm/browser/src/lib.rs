#![allow(incomplete_features)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{
    actors::{button::*, led::*},
    *,
};
use drogue_wasm::*;
use embassy::executor::Spawner;

struct MyDevice {
    led: ActorContext<'static, Led<WebLed>>,
    button: ActorContext<'static, Button<WebButton, Address<'static, Led<WebLed>>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

// Called when the wasm module is instantiated
#[embassy::main]
async fn main(spawner: Spawner) {
    wasm_logger::init(wasm_logger::Config::default());

    static mut INPUT1: InputPin = InputPin::new();
    static mut OUTPUT1: OutputPin = OutputPin::new();

    // Configure HTML elements
    unsafe {
        INPUT1.configure("button");
        OUTPUT1.configure("led", |value| {
            if value {
                log::info!("ON");
                OutputVisual::String("ON")
            } else {
                log::info!("OFF");
                OutputVisual::String("OFF")
            }
        });
    }

    let button = WebButton::new(unsafe { &INPUT1 });
    let led = WebLed::new(unsafe { &OUTPUT1 });

    DEVICE.configure(MyDevice {
        led: ActorContext::new(Led::new(led)),
        button: ActorContext::new(Button::new(button)),
    });

    DEVICE
        .mount(|device| async move {
            let led = device.led.mount((), spawner);
            device.button.mount(led, spawner);
        })
        .await;
}
