#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::{LedMatrixActor, MatrixCommand},
    drivers::led::matrix::{fonts, Frame, LedMatrix, ToFrame},
    traits::led::TextDisplay,
    ActorContext, DeviceContext,
};

use embassy::time::{Duration, Timer};
use embassy_nrf::{
    gpio::{AnyPin, Level, Output, OutputDrive, Pin},
    interrupt, twim, Peripherals,
};
use panic_probe as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    matrix: ActorContext<'static, AppMatrix>,
}
use lsm303agr::{Lsm303agr, MagOutputDataRate};

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
    let mut twi = twim::Twim::new(p.TWISPI0, irq, p.P0_16, p.P0_08, config);

    let mut sensor = Lsm303agr::new_with_i2c(twi);
    sensor.init().unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz100).unwrap();
    let mut sensor = sensor.into_mag_continuous().ok().unwrap();
    let mut direction = Direction::North;
    loop {
        let status = sensor.mag_status().unwrap();
        if status.xyz_new_data {
            let data = sensor.mag_data().unwrap();
            defmt::info!("{} {} {}", data.x, data.y, data.z);
        }
        direction = match direction {
            Direction::North => Direction::NorthEast,
            Direction::NorthEast => Direction::East,
            Direction::East => Direction::SouthEast,
            Direction::SouthEast => Direction::South,
            Direction::South => Direction::SouthWest,
            Direction::SouthWest => Direction::West,
            Direction::West => Direction::NorthWest,
            Direction::NorthWest => Direction::North,
        };

        defmt::info!("Applying direction");
        matrix
            .request(MatrixCommand::ApplyFrame(&direction))
            .unwrap()
            .await;

        Timer::after(Duration::from_millis(1000)).await;
    }
}

pub enum Direction {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl ToFrame<5, 5> for Direction {
    fn to_frame(&self) -> Frame<5, 5> {
        defmt::info!("CONVERTING FRAME");
        match self {
            #[rustfmt::skip]
            Direction::North => fonts::frame_5x5(&[0b00100, 0b01110, 0b10101, 0b00100, 0b00100]),
            #[rustfmt::skip]
            Direction::East =>
            {
                fonts::frame_5x5(&[
                    0b00100,
                    0b00010,
                    0b11111,
                    0b00010,
                    0b00100,
                ])
            },
            #[rustfmt::skip]
            Direction::West =>
            {
                fonts::frame_5x5(&[
                    0b00100,
                    0b01000,
                    0b11111,
                    0b01000,
                    0b00100,
                ])
            },
            #[rustfmt::skip]
            Direction::South =>
            {
                fonts::frame_5x5(&[
                    0b00100,
                    0b00100,
                    0b10101,
                    0b01110,
                    0b00100,
                ])
            },
            _ => fonts::frame_5x5(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
        }
    }
}
