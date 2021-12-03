#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use bsp_blinky_app::{BlinkyApp, BlinkyBoard};
use drogue_device::{
    bind_bsp, boot_bsp,
    drivers::led::{ActiveHigh, Led},
    DeviceContext,
};
use embassy_nrf::Peripherals;
use embedded_hal::digital::v2::OutputPin;

use defmt_rtt as _;
use drogue_device::bsp::boards::nrf52::microbit::{ButtonA, LedMatrix, Microbit};
use drogue_device::bsp::{boot, App, AppBoard};
use panic_probe as _;

// Creates a newtype named `BSP` around the `Iot02a` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(Microbit, BSP);

/// Handy type alias to make referencing easier.
type Configuration = <BlinkyApp<BSP> as App>::Configuration;

/// Define the required associated types for easy reference to avoid
/// generic explosion for the details of this board to the app.
impl BlinkyBoard for BSP {
    type Led = Led<BlinkyLed, ActiveHigh>;
    type ControlButton = ButtonA;
}

/// Create the matching configuration given the bound BSP.
impl AppBoard<BlinkyApp<Self>> for BSP {
    fn configure(self) -> Configuration {
        Configuration {
            led: Led::new(BlinkyLed(self.0.led_matrix)),
            control_button: self.0.button_a,
        }
    }
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    boot_bsp!(BlinkyApp, BSP, p, spawner);
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
