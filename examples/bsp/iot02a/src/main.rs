#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use bsp_blinky_app::{BlinkyApp, BlinkyBoard};
use drogue_device::{bind_bsp, boot_bsp, DeviceContext};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::Peripherals;

use defmt_rtt as _;
use drogue_device::bsp::{boot, App, AppBoard};
use drogue_device::bsp::boards::stm32u5::b_u585i_iot02a::{Iot02a, LedRed, UserButton};
use panic_probe as _;


// Creates a newtype named `BSP` around the `Iot02a` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(Iot02a, BSP);

/// Handy type alias to make referencing easier.
type Configuration = <BlinkyApp<BSP> as App>::Configuration;

/// Define the required associated types for easy reference to avoid
/// generic explosion for the details of this board to the app.
impl BlinkyBoard for BSP {
    type Led = LedRed;
    type ControlButton = UserButton;
}

/// Create the matching configuration given the bound BSP.
impl AppBoard<BlinkyApp<Self>> for BSP {
    fn take(self) -> Configuration {
        Configuration {
            led: self.0.led_red,
            control_button: self.0.user_button,
        }
    }
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    boot_bsp!(BlinkyApp, BSP, p, spawner);
}
