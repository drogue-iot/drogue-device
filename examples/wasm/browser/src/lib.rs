#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{
    actors::{button::*, led::*},
    drivers::led::Led as LedDriver,
};
use ector::*;
use embassy_executor::Spawner;
use web_embedded_hal::*;

type AppLed = LedDriver<WebLed>;

// Called when the wasm module is instantiated
#[embassy_executor::main]
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

    static BUTTON: ActorContext<Button<WebButton, LedMessage>> = ActorContext::new();
    static LED: ActorContext<Led<AppLed>> = ActorContext::new();

    let led = LED.mount(
        spawner,
        Led::new(LedDriver::new(WebLed::new(unsafe { &OUTPUT1 }))),
    );
    BUTTON.mount(
        spawner,
        Button::new(WebButton::new(unsafe { &INPUT1 }), led),
    );
}
