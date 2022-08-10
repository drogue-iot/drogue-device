#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use actors::led::LedMessage;
use defmt_rtt as _;
use drogue_device::{actors, drivers};
use ector::*;
use embassy_rp::{
    gpio::{Level, Output},
    peripherals::PIN_25,
    Peripherals,
};

use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::executor::Spawner, p: Peripherals) {
    static LED: ActorContext<actors::led::Led<drivers::led::Led<Output<'static, PIN_25>>>> =
        ActorContext::new();
    let led = LED.mount(
        spawner,
        actors::led::Led::new(drivers::led::Led::new(Output::new(p.PIN_25, Level::Low))),
    );

    loop {
        cortex_m::asm::delay(1_000_000);
        led.notify(LedMessage::Toggle).await;
    }
}
