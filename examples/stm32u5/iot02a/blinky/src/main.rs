#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use drogue_blinky_app::{BlinkyBoard, BlinkyConfiguration, BlinkyDevice};
use drogue_device::{bind_bsp, Board};
use embassy_util::Forever;
use embassy_stm32::Peripherals;

use defmt_rtt as _;
use drogue_device::bsp::boards::stm32u5::b_u585i_iot02a::{Iot02a, LedRed, UserButton};
use panic_probe as _;

// Creates a newtype named `BSP` around the `Iot02a` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(Iot02a, BSP);

/// Define the required associated types for easy reference to avoid
/// generic explosion for the details of this board to the app.
impl BlinkyBoard for BSP {
    type Led = LedRed;
    type ControlButton = UserButton;
}

static DEVICE: Forever<BlinkyDevice<BSP>> = Forever::new();

#[embassy_executor::main]
async fn main(spawner: embassy_executor::executor::Spawner, p: Peripherals) {
    let board = BSP::new(p);

    let config = BlinkyConfiguration {
        led: board.0.led_red,
        control_button: board.0.user_button,
    };
    DEVICE.put(BlinkyDevice::new()).mount(spawner, config).await;
}
