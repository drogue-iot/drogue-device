#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use defmt_rtt as _;
use drogue_device::{actors::led::*, ActorContext, DeviceContext};
use embassy_rp::{
    gpio::{Level, Output},
    peripherals::PIN_25,
    Peripherals,
};

use panic_probe as _;

pub struct MyDevice {
    led: ActorContext<'static, Led<Output<'static, PIN_25>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    DEVICE.configure(MyDevice {
        led: ActorContext::new(Led::new(Output::new(p.PIN_25, Level::Low))),
    });

    let led = DEVICE.mount(|device| device.led.mount((), spawner.into()));

    loop {
        cortex_m::asm::delay(1_000_000);
        led.request(LedMessage::Toggle).unwrap().await;
    }
}
