mod register;
use crate::domain::temperature::{Celsius, Temperature, TemperatureScale};
use crate::traits::i2c::I2cAddress;
use core::fmt::{Debug, Formatter};
use embassy::traits::i2c::*;
use register::calibration::*;
use register::ctrl1::{BlockDataUpdate, Ctrl1, OutputDataRate};
use register::ctrl2::Ctrl2;
use register::ctrl3::Ctrl3;
use register::h_out::Hout;
use register::status::Status;
use register::t_out::Tout;

#[derive(Copy, Clone)]
pub struct SensorAcquisition<S: TemperatureScale> {
    pub temperature: Temperature<S>,
    pub relative_humidity: f32,
}

impl<S: TemperatureScale> Debug for SensorAcquisition<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SensorAcquisition")
            .field("temperature", &self.temperature)
            .field("relative_humidity", &self.relative_humidity)
            .finish()
    }
}

#[cfg(feature = "defmt")]
impl<S: TemperatureScale> defmt::Format for SensorAcquisition<S> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(
            f,
            "SensorAcquisition(temperature: {}, relative_humidity: {})",
            &self.temperature,
            &self.relative_humidity
        );
    }
}

pub const ADDR: u8 = 0x5F;

pub enum Hts221Error<E> {
    I2c(E),
    NotCalibrated,
}

pub struct Hts221 {
    address: I2cAddress,
    calibration: Option<Calibration>,
}

impl Hts221 {
    pub fn new() -> Self {
        Self {
            address: I2cAddress::new(ADDR),
            calibration: None,
        }
    }

    pub async fn initialize<I: I2c<SevenBitAddress>>(
        &mut self,
        i2c: &mut I,
    ) -> Result<(), Hts221Error<I::Error>> {
        Ctrl2::modify(self.address, i2c, |reg| {
            reg.boot();
        })
        .await?;

        Ctrl1::modify(self.address, i2c, |reg| {
            reg.power_active()
                .output_data_rate(OutputDataRate::Hz1)
                .block_data_update(BlockDataUpdate::MsbLsbReading);
        })
        .await?;

        Ctrl3::modify(self.address, i2c, |reg| {
            reg.enable(true);
        })
        .await?;

        loop {
            // Ensure status is emptied
            if let Ok(status) = Status::read(self.address, i2c).await {
                if !status.any_available() {
                    break;
                }
            }
            Hout::read(self.address, i2c).await?;
            Tout::read(self.address, i2c).await?;
        }

        self.calibration
            .replace(Calibration::read(self.address, i2c).await?);
        Ok(())
    }

    pub async fn read<I: I2c<SevenBitAddress>>(
        &mut self,
        i2c: &mut I,
    ) -> Result<SensorAcquisition<Celsius>, Hts221Error<I::Error>> {
        if let Some(calibration) = &self.calibration {
            let t_out = Tout::read(self.address, i2c).await? as i16;
            let temperature = calibration.calibrated_temperature(t_out);

            let h_out = Hout::read(self.address, i2c).await?;
            let relative_humidity = calibration.calibrated_humidity(h_out);

            Ok(SensorAcquisition {
                temperature,
                relative_humidity,
            })
        } else {
            Err(Hts221Error::NotCalibrated)
        }
    }
}

impl<E> From<E> for Hts221Error<E> {
    fn from(e: E) -> Hts221Error<E> {
        Hts221Error::I2c(e)
    }
}
