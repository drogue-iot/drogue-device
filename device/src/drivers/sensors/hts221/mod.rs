mod register;
use crate::domain::{temperature::Celsius, SensorAcquisition};
use crate::traits::{i2c::I2cAddress, sensors::temperature::TemperatureSensor};
use core::future::Future;
use embassy::traits::i2c::*;
use register::calibration::*;
use register::ctrl1::{BlockDataUpdate, Ctrl1, OutputDataRate};
use register::ctrl2::Ctrl2;
use register::ctrl3::Ctrl3;
use register::h_out::Hout;
use register::status::Status;
use register::t_out::Tout;

pub const ADDR: u8 = 0x5F;

pub enum Hts221Error<E> {
    I2c(E),
    NotCalibrated,
}

pub struct Hts221<I>
where
    I: I2c<SevenBitAddress> + 'static,
    <I as I2c>::Error: Send,
{
    i2c: I,
    address: I2cAddress,
    calibration: Option<Calibration>,
}

impl<I> Hts221<I>
where
    I: I2c<SevenBitAddress> + 'static,
    <I as I2c>::Error: Send,
{
    pub fn new(i2c: I) -> Self {
        Self {
            i2c,
            address: I2cAddress::new(ADDR),
            calibration: None,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), Hts221Error<I::Error>> {
        Ctrl2::modify(self.address, &mut self.i2c, |reg| {
            reg.boot();
        })
        .await?;

        Ctrl1::modify(self.address, &mut self.i2c, |reg| {
            reg.power_active()
                .output_data_rate(OutputDataRate::Hz1)
                .block_data_update(BlockDataUpdate::MsbLsbReading);
        })
        .await?;

        Ctrl3::modify(self.address, &mut self.i2c, |reg| {
            reg.enable(true);
        })
        .await?;

        loop {
            // Ensure status is emptied
            if let Ok(status) = Status::read(self.address, &mut self.i2c).await {
                if !status.any_available() {
                    break;
                }
            }
            Hout::read(self.address, &mut self.i2c).await?;
            Tout::read(self.address, &mut self.i2c).await?;
        }

        self.calibration
            .replace(Calibration::read(self.address, &mut self.i2c).await?);
        Ok(())
    }

    pub async fn read(&mut self) -> Result<SensorAcquisition<Celsius>, Hts221Error<I::Error>> {
        if let Some(calibration) = &self.calibration {
            let t_out = Tout::read(self.address, &mut self.i2c).await? as i16;
            let temperature = calibration.calibrated_temperature(t_out);

            let h_out = Hout::read(self.address, &mut self.i2c).await?;
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

impl<I> TemperatureSensor<Celsius> for Hts221<I>
where
    I: I2c<SevenBitAddress> + 'static,
    <I as I2c>::Error: Send,
{
    type Error = Hts221Error<<I as I2c>::Error>;

    type CalibrateFuture<'m>
    where
        I: 'm,
    = impl Future<Output = Result<(), Self::Error>> + 'm;

    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m> {
        self.initialize()
    }

    type ReadFuture<'m>
    where
        I: 'm,
    = impl Future<Output = Result<SensorAcquisition<Celsius>, Self::Error>> + 'm;

    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m> {
        self.read()
    }
}

impl<E> From<E> for Hts221Error<E>
where
    E: Send,
{
    fn from(e: E) -> Hts221Error<E> {
        Hts221Error::I2c(e)
    }
}
