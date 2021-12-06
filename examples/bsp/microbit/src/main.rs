#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use bsp_blinky_app::{BlinkyBoard, BlinkyConfiguration, BlinkyDevice};
use drogue_device::{
    drivers::led::{ActiveHigh, Led},
    DeviceContext,
};
use embassy_nrf::Peripherals;
use embedded_hal::digital::v2::OutputPin;

use defmt_rtt as _;
use drogue_device::bsp::boards::nrf52::microbit::{ButtonA, LedMatrix, Microbit};
use panic_probe as _;

pub struct MyBoard(Microbit);

impl BlinkyBoard for MyBoard {
    type Led = Led<BlinkyLed, ActiveHigh>;
    type ControlButton = ButtonA;
}

static DEVICE: DeviceContext<BlinkyDevice<MyBoard>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = MyBoard(Microbit::new(p));

    DEVICE.configure(BlinkyDevice::new(BlinkyConfiguration {
        led: Led::new(BlinkyLed(board.0.led_matrix)),
        control_button: board.0.button_a,
    }));

    DEVICE.mount(|device| device.mount(spawner)).await;
}

pub struct BlinkyLed(LedMatrix);

impl OutputPin for BlinkyLed {
    type Error = ();
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.0.set_row_high(0);
        self.0.set_col_high(0);
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.0.set_row_low(0);
        self.0.set_col_low(0);
        Ok(())
    }
}
