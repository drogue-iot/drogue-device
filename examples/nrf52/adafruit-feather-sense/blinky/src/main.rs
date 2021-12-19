#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    bind_bsp,
    bsp::boards::nrf52::adafruit_feather_sense::{AdafruitFeatherSense, LedRed, UserButton},
    Board, DeviceContext,
};

use bsp_blinky_app::{BlinkyBoard, BlinkyConfiguration, BlinkyDevice};
use embassy_nrf::Peripherals;

bind_bsp!(AdafruitFeatherSense, BSP);

/// Define the required associated types for easy reference to avoid
/// generic explosion for the details of this board to the app.
impl BlinkyBoard for BSP {
    type Led = LedRed;
    type ControlButton = UserButton;
}

static DEVICE: DeviceContext<BlinkyDevice<BSP>> = DeviceContext::new();
//
// Application must run at a lower priority than softdevice
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;

fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = BSP::new(p);

    DEVICE
        .configure(BlinkyDevice::new())
        .mount(
            spawner,
            BlinkyConfiguration {
                led: board.0.led_red,
                control_button: board.0.user_button,
            },
        )
        .await;
}
