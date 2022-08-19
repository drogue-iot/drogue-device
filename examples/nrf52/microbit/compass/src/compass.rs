use core::fmt::Debug;
use drogue_device::bsp::boards::nrf52::microbit::LedMatrix;
use drogue_device::domain::led::matrix::Frame;
use drogue_device::drivers::led::matrix::fonts;
use drogue_device::traits::led::ToFrame;
use embassy_time::{Duration, Timer};
use lsm303agr::{
    interface::{ReadData, WriteData},
    mode::MagContinuous,
    Error, Lsm303agr,
};
use micromath::F32Ext;

pub struct MicrobitCompass<I, E1, E2>
where
    I: WriteData<Error = Error<E1, E2>> + ReadData<Error = Error<E1, E2>>,
    E1: Debug,
    E2: Debug,
{
    heading_offset: i32,
    x_offset: i32,
    y_offset: i32,
    x_scale: f32,
    y_scale: f32,
    sensor: Lsm303agr<I, MagContinuous>,
}

impl<I, E1, E2> MicrobitCompass<I, E1, E2>
where
    I: WriteData<Error = Error<E1, E2>> + ReadData<Error = Error<E1, E2>>,
    E1: Debug,
    E2: Debug,
{
    pub fn new(sensor: Lsm303agr<I, MagContinuous>, heading_offset: i32) -> Self {
        Self {
            sensor,
            heading_offset,
            x_offset: 0,
            y_offset: 0,
            x_scale: 1.0,
            y_scale: 1.0,
        }
    }

    /// Calibrate the microbit using a simple game idea from the official microbit firmware. User
    /// have to move the microbit around to light up all LEDs, and we will sample min and max values
    /// until done.
    pub async fn calibrate(&mut self, matrix: &mut LedMatrix) {
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

        matrix.clear();
        let (mut min_x, mut min_y, mut min_z) = (i32::MAX, i32::MAX, i32::MAX);
        let (mut max_x, mut max_y, mut max_z) = (i32::MIN, i32::MIN, i32::MIN);
        let mut samples = 0;
        while samples < PERIMETER_POINTS {
            if self.sensor.accel_status().unwrap().xyz_new_data {
                let data = self.sensor.accel_data().unwrap();

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

            defmt::trace!("Cursor: ({}, {})", cursor.0, cursor.1);

            // Sample some data
            let status = self.sensor.mag_status().unwrap();
            if status.xyz_new_data {
                let data = self.sensor.mag_data().unwrap();
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
                    visited[i] = true;
                    matrix.on(PERIMETER[i].0, PERIMETER[i].1);
                    samples += 1;
                }
            }
            matrix.render();

            Timer::after(Duration::from_micros(500)).await;
        }

        defmt::info!("Calibration complete!");
        matrix.clear();

        self.x_offset = (min_x + max_x) / 2;
        self.y_offset = (min_y + max_y) / 2;

        self.x_scale = 1.0 / (max_x - min_x) as f32;
        self.y_scale = 1.0 / (max_y - min_y) as f32;

        defmt::trace!(
        "Calibrated values (x, y) offset({}, {}), scale({}, {}): x({}, {}), y({}, {}), z({}, {})",
        self.x_offset,
        self.y_offset,
        self.x_scale,
        self.y_scale,
        min_x,
        max_x,
        min_y,
        max_y,
        min_z,
        max_z
    );
    }

    pub async fn heading(&mut self) -> i32 {
        loop {
            let status = self.sensor.mag_status().unwrap();
            if status.xyz_new_data {
                let data = self.sensor.mag_data().unwrap();
                // Normalize for within calibrated range
                let x_sample = ((data.x - self.x_offset) as f32) * self.x_scale;
                let y_sample = ((data.y - self.y_offset) as f32) * self.y_scale;

                let x: f32 = -y_sample;
                let y: f32 = x_sample;

                defmt::trace!("Coord ({}, {})", x, y);

                let mut heading =
                    (x.atan2(y) * 180.0 / core::f32::consts::PI) + self.heading_offset as f32;
                if heading < 0.0 {
                    heading += 360.0;
                }
                defmt::trace!("Heading: {}", heading);

                return heading as i32;
            } else {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}

impl From<i32> for Direction {
    fn from(heading: i32) -> Self {
        let heading = heading % 360;
        if heading >= 340 || heading < 25 {
            Direction::North
        } else if heading >= 25 && heading < 70 {
            Direction::NorthEast
        } else if heading >= 70 && heading < 115 {
            Direction::East
        } else if heading >= 115 && heading < 160 {
            Direction::SouthEast
        } else if heading >= 160 && heading < 205 {
            Direction::South
        } else if heading >= 205 && heading < 250 {
            Direction::SouthWest
        } else if heading >= 250 && heading < 295 {
            Direction::West
        } else if heading >= 295 && heading < 340 {
            Direction::NorthWest
        } else {
            panic!("Cannot happen!");
        }
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
