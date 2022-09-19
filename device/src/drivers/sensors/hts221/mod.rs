mod register;
use crate::{
    domain::{temperature::Celsius, SensorAcquisition},
    traits::{i2c::I2cAddress, sensors::temperature::TemperatureSensor},
};
use core::future::Future;
use embedded_hal_async::i2c::*;
use register::{
    calibration::*,
    ctrl1::{BlockDataUpdate, Ctrl1, OutputDataRate},
    ctrl2::Ctrl2,
    ctrl3::Ctrl3,
    h_out::Hout,
    status::Status,
    t_out::Tout,
};

pub const ADDR: u8 = 0x5F;

pub enum Hts221Error<E> {
    I2c(E),
    NotCalibrated,
}

pub struct Hts221<I>
where
    I: I2c<SevenBitAddress> + 'static,
    <I as ErrorType>::Error: Send,
{
    i2c: I,
    address: I2cAddress,
    calibration: Option<Calibration>,
}

impl<I> Hts221<I>
where
    I: I2c<SevenBitAddress> + 'static,
    <I as ErrorType>::Error: Send,
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
    <I as ErrorType>::Error: Send,
{
    type Error = Hts221Error<<I as ErrorType>::Error>;

    type CalibrateFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where I: 'm;

    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m> {
        self.initialize()
    }

    type ReadFuture<'m> = impl Future<Output = Result<SensorAcquisition<Celsius>, Self::Error>> + 'm where I: 'm;

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
