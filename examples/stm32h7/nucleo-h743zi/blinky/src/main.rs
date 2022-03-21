#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use drogue_blinky_app::{BlinkyBoard, BlinkyConfiguration, BlinkyDevice};
use drogue_device::{bind_bsp, Board, DeviceContext};
use embassy_stm32::Peripherals;

use defmt_rtt as _;
use drogue_device::bsp::boards::stm32h7::nucleo_h743zi::*;
use panic_probe as _;

// Creates a newtype named `BSP` around the `NucleoH743` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(NucleoH743, BSP);

/// Define the required associated types for easy reference to avoid
/// generic explosion for the details of this board to the app.
impl BlinkyBoard for BSP {
    type Led = LedGreen;
    type ControlButton = UserButton;
}

static DEVICE: DeviceContext<BlinkyDevice<BSP>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = BSP::new(p);

    DEVICE
        .configure(BlinkyDevice::new())
        .mount(
            spawner,
            BlinkyConfiguration {
                led: board.0.led_green,
                control_button: board.0.user_button,
            },
        )
        .await;
}
