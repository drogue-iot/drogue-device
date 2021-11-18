#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::LedMatrixActor,
    drivers::led::matrix::LedMatrix,
    traits::led::{LedMatrix as LedMatrixTrait, TextDisplay},
    ActorContext, DeviceContext,
};

use embassy::time::{Duration, Timer};
use embassy_nrf::{
    gpio::{AnyPin, Level, Output, OutputDrive, Pin},
    interrupt, twim, Peripherals,
};

mod compass;
use compass::*;

use panic_probe as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    matrix: ActorContext<'static, AppMatrix>,
}
use lsm303agr::{AccelOutputDataRate, Lsm303agr, MagOutputDataRate};

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    // LED Matrix
    let rows = [
        output_pin(p.P0_21.degrade()),
        output_pin(p.P0_22.degrade()),
        output_pin(p.P0_15.degrade()),
        output_pin(p.P0_24.degrade()),
        output_pin(p.P0_19.degrade()),
    ];

    let cols = [
        output_pin(p.P0_28.degrade()),
        output_pin(p.P0_11.degrade()),
        output_pin(p.P0_31.degrade()),
        output_pin(p.P1_05.degrade()),
        output_pin(p.P0_30.degrade()),
    ];
    let led = LedMatrix::new(rows, cols);

    DEVICE.configure(MyDevice {
        matrix: ActorContext::new(LedMatrixActor::new(Duration::from_millis(1000 / 200), led)),
    });

    let mut matrix = DEVICE
        .mount(|device| async move {
            let matrix = device.matrix.mount((), spawner);
            matrix
        })
        .await;

    matrix.scroll("Yo!").await.unwrap();

    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let twi = twim::Twim::new(p.TWISPI0, irq, p.P0_16, p.P0_08, config);

    let mut sensor = Lsm303agr::new_with_i2c(twi);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz100).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz100).unwrap();
    let sensor = sensor.into_mag_continuous().ok().unwrap();

    // Use heading offset of 90 which seems accurate during testing
    let mut compass = MicrobitCompass::new(sensor, 90);
    compass.calibrate(&mut matrix).await;

    loop {
        let direction: Direction = compass.heading().await.into();
        defmt::trace!("Direction: {:?}", direction);
        matrix.apply(&direction).await.unwrap();

        Timer::after(Duration::from_millis(10)).await;
    }
}
