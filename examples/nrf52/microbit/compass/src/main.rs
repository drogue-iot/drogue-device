#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::LedMatrixActor,
    bsp::boards::nrf52::microbit::*,
    traits::led::{LedMatrix as LedMatrixTrait, TextDisplay},
    ActorContext, Board,
};

use embassy::time::{Duration, Timer};
use embassy_nrf::{
    gpio::{AnyPin, Output},
    interrupt, twim, Peripherals,
};
use lsm303agr::{AccelOutputDataRate, Lsm303agr, MagOutputDataRate};

mod compass;
use compass::*;

use panic_probe as _;

static LED_MATRIX: ActorContext<LedMatrixActor<Output<'static, AnyPin>, 5, 5>> =
    ActorContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let mut matrix = LED_MATRIX.mount(spawner, LedMatrixActor::new(board.display, None));

    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let twi = twim::Twim::new(board.twispi0, irq, board.p23, board.p22, config);

    let mut sensor = Lsm303agr::new_with_i2c(twi);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz50).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz50).unwrap();
    let sensor = sensor.into_mag_continuous().ok().unwrap();

    // Use heading offset of 90 which seems accurate during testing
    let mut compass = MicrobitCompass::new(sensor, 90);
    matrix
        .scroll("Move micro:bit until LEDs are lit")
        .await
        .unwrap();
    compass.calibrate(&mut matrix).await;

    loop {
        let direction: Direction = compass.heading().await.into();
        defmt::trace!("Direction: {:?}", direction);
        matrix.apply(&direction).await.unwrap();

        Timer::after(Duration::from_millis(10)).await;
    }
}
