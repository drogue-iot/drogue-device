//! Accelerometer for the micro:bit
use embassy_nrf::{
    interrupt::SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0,
    peripherals::{P0_08, P0_16, TWISPI0},
    twim, Peripheral,
};
use lsm303agr::{
    interface::I2cInterface, mode::MagOneShot, AccelMode, AccelOutputDataRate, Error as LsmError,
    Lsm303agr, Measurement, Status,
};

type I2C<'d> = twim::Twim<'d, TWISPI0>;

/// Accelerometer error
pub type Error = LsmError<twim::Error, ()>;

/// Accelerometer peripheral present on the microbit
pub struct Accelerometer<'d> {
    sensor: Lsm303agr<I2cInterface<I2C<'d>>, MagOneShot>,
}

impl<'d> Accelerometer<'d> {
    /// Create and initialize the accelerometer
    pub fn new(
        twispi0: TWISPI0,
        irq: impl Peripheral<P = SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0> + 'd,
        sda: P0_16,
        scl: P0_08,
    ) -> Result<Self, Error> {
        let config = twim::Config::default();
        let twi = twim::Twim::new(twispi0, irq, sda, scl, config);

        let mut sensor = Lsm303agr::new_with_i2c(twi);
        sensor.init()?;
        sensor.set_accel_odr(AccelOutputDataRate::Hz50)?;
        sensor.set_accel_mode(AccelMode::Normal)?;

        Ok(Self { sensor })
    }

    /// Return status of accelerometer
    pub fn accel_status(&mut self) -> Result<Status, Error> {
        self.sensor.accel_status()
    }

    /// Return accelerometer data
    ///
    /// Returned in mg (milli-g) where 1g is 9.8m/s².
    pub fn accel_data(&mut self) -> Result<Measurement, Error> {
        self.sensor.accel_data()
    }
}
