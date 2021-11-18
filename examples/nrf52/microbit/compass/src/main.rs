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

use embassy::time::{Duration, Instant, Timer};
use embassy::traits::gpio::WaitForLow;
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    gpiote::PortInput,
    interrupt, twim, Peripherals,
};
use heapless::Vec;
use micromath::F32Ext;

use panic_probe as _;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    matrix: ActorContext<'static, AppMatrix>,
}
use embedded_hal::digital::v2::InputPin;
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
    let mut twi = twim::Twim::new(p.TWISPI0, irq, p.P0_16, p.P0_08, config);

    let mut sensor = Lsm303agr::new_with_i2c(twi);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz100).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz100).unwrap();

    let mut sensor = sensor.into_mag_continuous().ok().unwrap();
    let mut direction = Direction::North;

    let mut button_a = PortInput::new(Input::new(p.P0_14, Pull::Up));
    let mut button_b = PortInput::new(Input::new(p.P0_23, Pull::Up));

    defmt::info!("Calibrating... move the microbit around until all pixels are lit");

    pub struct Point(pub usize, pub usize);
    const PERIMETER_POINTS: usize = 25;
    const PERIMETER: [Point; PERIMETER_POINTS] = [
        Point(0, 0),
        Point(0, 1),
        Point(0, 2),
        Point(0, 3),
        Point(0, 4),
        Point(1, 0),
        Point(1, 1),
        Point(1, 2),
        Point(1, 3),
        Point(1, 4),
        Point(2, 0),
        Point(2, 1),
        Point(2, 2),
        Point(2, 3),
        Point(2, 4),
        Point(3, 0),
        Point(3, 1),
        Point(3, 2),
        Point(3, 3),
        Point(3, 4),
        Point(4, 0),
        Point(4, 1),
        Point(4, 2),
        Point(4, 3),
        Point(4, 4),
    ];

    const PIXEL1_THRESHOLD: i32 = 200;
    const PIXEL2_THRESHOLD: i32 = 680;
    let mut visited = [false; PERIMETER_POINTS];
    let mut cursor = Point(2, 2);
    matrix.clear().await;

    let (mut min_x, mut min_y, mut min_z) = (i32::MAX, i32::MAX, i32::MAX);
    let (mut max_x, mut max_y, mut max_z) = (i32::MIN, i32::MIN, i32::MIN);
    let mut samples = 0;
    while samples < PERIMETER_POINTS {
        if sensor.accel_status().unwrap().xyz_new_data {
            let data = sensor.accel_data().unwrap();

            if data.x < -PIXEL2_THRESHOLD {
                cursor.0 = 0;
            } else if data.x < -PIXEL1_THRESHOLD {
                cursor.0 = 1;
            } else if data.x > PIXEL2_THRESHOLD {
                cursor.0 = 4;
            } else if data.x > PIXEL1_THRESHOLD {
                cursor.0 = 3;
            } else {
                cursor.0 = 2;
            }

            if data.y < -PIXEL2_THRESHOLD {
                cursor.1 = 0;
            } else if data.y < -PIXEL1_THRESHOLD {
                cursor.1 = 1;
            } else if data.y > PIXEL2_THRESHOLD {
                cursor.1 = 4;
            } else if data.y > PIXEL1_THRESHOLD {
                cursor.1 = 3;
            } else {
                cursor.1 = 2;
            }
        }

        defmt::info!("Cursor: ({}, {})", cursor.0, cursor.1);

        // Sample some data
        let status = sensor.mag_status().unwrap();
        if status.xyz_new_data {
            let data = sensor.mag_data().unwrap();
            max_x = core::cmp::max(max_x, data.x);
            max_y = core::cmp::max(max_y, data.y);
            max_z = core::cmp::max(max_z, data.z);

            min_x = core::cmp::min(min_x, data.x);
            min_y = core::cmp::min(min_y, data.y);
            min_z = core::cmp::min(min_z, data.z);
        }

        // Update visited state
        for i in 0..PERIMETER_POINTS {
            if cursor.0 == PERIMETER[i].0 && cursor.1 == PERIMETER[i].1 && !visited[i] {
                let status = sensor.mag_status().unwrap();
                if status.xyz_new_data {
                    visited[i] = true;
                    matrix.on(PERIMETER[i].0, PERIMETER[i].1).await;
                    samples += 1;
                }
            }
        }

        Timer::after(Duration::from_millis(10)).await;
    }

    defmt::info!("Calibration complete!");
    matrix.clear().await;

    let x_offset = (min_x + max_x) / 2;
    let y_offset = (min_y + max_y) / 2;

    let x_scale = 1.0 / (max_x - min_x) as f32;
    let y_scale = 1.0 / (max_y - min_y) as f32;

    defmt::info!(
        "Calibrated values (x, y) offset({}, {}), scale({}, {}): x({}, {}), y({}, {}), z({}, {})",
        x_offset,
        y_offset,
        x_scale,
        y_scale,
        min_x,
        max_x,
        min_y,
        max_y,
        min_z,
        max_z
    );

    loop {
        let status = sensor.mag_status().unwrap();
        if status.xyz_new_data {
            let data = sensor.mag_data().unwrap();
            // Normalize for within calibrated range
            let x_sample = ((data.x - x_offset) as f32) * x_scale;
            let y_sample = ((data.y - y_offset) as f32) * y_scale;

            let x: f32 = -y_sample;
            let y: f32 = x_sample;

            defmt::info!("Coord ({}, {})", x, y);

            let mut heading = (x.atan2(y) * 180.0 / core::f32::consts::PI) + 90.0;
            if heading < 0.0 {
                heading += 360.0;
            }
            defmt::info!("Heading: {}", heading);

            let heading = heading as i32;
            if heading >= 340 || heading < 25 {
                direction = Direction::North;
            } else if heading >= 25 && heading < 70 {
                direction = Direction::NorthEast;
            } else if heading >= 70 && heading < 115 {
                direction = Direction::East;
            } else if heading >= 115 && heading < 160 {
                direction = Direction::SouthEast;
            } else if heading >= 160 && heading < 205 {
                direction = Direction::South;
            } else if heading >= 205 && heading < 250 {
                direction = Direction::SouthWest;
            } else if heading >= 250 && heading < 295 {
                direction = Direction::West
            } else if heading >= 295 && heading < 340 {
                direction = Direction::NorthWest
            }
        }
        /*
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
        */
        defmt::info!("Direction: {:?}", direction);

        matrix
            .request(MatrixCommand::ApplyFrame(&direction))
            .unwrap()
            .await;

        Timer::after(Duration::from_millis(100)).await;
    }
}

#[derive(Debug, defmt::Format)]
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
        match self {
            #[rustfmt::skip]
            Direction::North => {
                fonts::frame_5x5(&[
                    0b00100,
                    0b00100,
                    0b10101,
                    0b01110,
                    0b00100,
                ])
            },
            #[rustfmt::skip]
            Direction::NorthEast =>
            {
                fonts::frame_5x5(&[
                    0b00001,
                    0b00010,
                    0b10100,
                    0b11000,
                    0b11100,
                ])
            },
            #[rustfmt::skip]
            Direction::East =>
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
            Direction::SouthEast =>
            {
                fonts::frame_5x5(&[
                    0b11100,
                    0b11000,
                    0b10100,
                    0b00010,
                    0b00001,
                ])
            },

            #[rustfmt::skip]
            Direction::South =>
            {
                fonts::frame_5x5(&[
                    0b00100,
                    0b01110,
                    0b10101,
                    0b00100,
                    0b00100
                ])
            },
            #[rustfmt::skip]
            Direction::SouthWest =>
            {
                fonts::frame_5x5(&[
                    0b00111,
                    0b00011,
                    0b00101,
                    0b01000,
                    0b10000,
                ])
            },
            #[rustfmt::skip]
            Direction::West =>
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
            Direction::NorthWest =>
            {
                fonts::frame_5x5(&[
                    0b10000,
                    0b01000,
                    0b00101,
                    0b00011,
                    0b00111,
                ])
            },
        }
    }
}
