#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::traits::led::ToFrame;
use drogue_device::{bsp::boards::nrf52::microbit::*, Board};

use embassy_executor::time::Duration;
use embassy_nrf::{interrupt, twim, Peripherals};
use lsm303agr::{AccelOutputDataRate, Lsm303agr, MagOutputDataRate};

mod compass;
use compass::*;

use panic_probe as _;

#[embassy_executor::main]
async fn main(_s: embassy_executor::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let twi = twim::Twim::new(board.twispi0, irq, board.p23, board.p22, config);

    let mut sensor = Lsm303agr::new_with_i2c(twi);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz50).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz50).unwrap();
    let sensor = sensor.into_mag_continuous().ok().unwrap();

    // Use heading offset of 90 which seems accurate during testing
    let mut display = board.display;
    let mut compass = MicrobitCompass::new(sensor, 90);
    display
        .scroll_with_speed("Move micro:bit until LEDs are lit", Duration::from_secs(10))
        .await;
    compass.calibrate(&mut display).await;

    loop {
        let direction: Direction = compass.heading().await.into();
        defmt::trace!("Direction: {:?}", direction);
        display
            .display(direction.to_frame(), Duration::from_millis(500))
            .await;
    }
}
