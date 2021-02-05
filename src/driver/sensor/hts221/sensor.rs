use crate::bind::Bind;
use crate::domain::temperature::Celsius;
use crate::driver::sensor::hts221::ready::DataReady;
use crate::driver::sensor::hts221::register::calibration::*;
use crate::driver::sensor::hts221::register::ctrl1::{BlockDataUpdate, Ctrl1, OutputDataRate};
use crate::driver::sensor::hts221::register::ctrl2::Ctrl2;
use crate::driver::sensor::hts221::register::ctrl3::Ctrl3;
use crate::driver::sensor::hts221::register::h_out::Hout;
use crate::driver::sensor::hts221::register::status::Status;
use crate::driver::sensor::hts221::register::t_out::Tout;
use crate::driver::sensor::hts221::SensorAcquisition;
use crate::hal::i2c::I2cAddress;
use crate::handler::EventHandler;
use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use crate::driver::i2c::I2cPeripheral;

pub const ADDR: u8 = 0x5F;

pub struct Sensor<D, I>
where
    D: Device + 'static,
    I: WriteRead + Read + Write + 'static,
{
    address: I2cAddress,
    i2c: Option<Address<I2cPeripheral<I>>>,
    calibration: Option<Calibration>,
    bus: Option<Address<EventBus<D>>>,
}

impl<D, I> Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write + 'static,
{
    pub fn new() -> Self {
        Self {
            address: I2cAddress::new(ADDR),
            i2c: None,
            calibration: None,
            bus: None,
        }
    }
}

impl<D, I> Default for Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write + 'static,
{
    fn default() -> Self {
        Sensor::new()
    }
}

impl<D, I> Actor for Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write,
{
    fn on_initialize(self) -> Completion<Self> {
        Completion::defer(async move {
            if let Some(i2c) = self.i2c {
                Ctrl2::modify(self.address, i2c, |reg| {
                    reg.boot();
                }).await.ok();

                Ctrl1::modify(self.address, i2c, |reg| {
                    reg.power_active()
                        .output_data_rate(OutputDataRate::Hz1)
                        .block_data_update(BlockDataUpdate::MsbLsbReading);
                }).await.ok();

                Ctrl3::modify(self.address, i2c, |reg| {
                    reg.enable(true);
                }).await.ok();

                loop {
                    // Ensure status is emptied
                    if let Ok(status) = Status::read(self.address, i2c).await {
                        if !status.any_available() {
                            break;
                        }
                    }
                    Hout::read(self.address, i2c).await.ok();
                    Tout::read(self.address, i2c).await.ok();
                }
            }
            self
        })
    }

    fn on_start(mut self) -> Completion<Self> {
        Completion::defer(async move {
            if let Some(i2c) = self.i2c {
                if let Ok(calibration) = Calibration::read(self.address, i2c).await {
                    self.calibration.replace( calibration );
                }
            }
            self
        })
    }
}

impl<D, I> Bind<EventBus<D>> for Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write,
{
    fn on_bind(&mut self, address: Address<EventBus<D>>) {
        self.bus.replace(address);
    }
}

impl<D, I> Bind<I2cPeripheral<I>> for Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write + 'static,
{
    fn on_bind(&mut self, address: Address<I2cPeripheral<I>>) {
        self.i2c.replace(address);
    }
}

impl<D, I> NotifyHandler<DataReady> for Sensor<D, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>>,
    I: WriteRead + Read + Write,
{
    fn on_notify(self, message: DataReady) -> Completion<Self> {
        Completion::defer(async move {
            if self.i2c.is_some() {
                let i2c = self.i2c.unwrap();

                if let Some(ref calibration) = self.calibration {
                    if let Ok(t_out) = Tout::read(self.address, i2c).await {
                        let temperature = calibration.calibrated_temperature(t_out);

                        if let Ok(h_out) = Hout::read(self.address, i2c).await {
                            let relative_humidity = calibration.calibrated_humidity(h_out);

                            self.bus.unwrap().publish(SensorAcquisition {
                                temperature,
                                relative_humidity,
                            });
                        }
                    }
                } else {
                    log::warn!("[hts221] no calibration data available")
                }
            }
            self
        })
    }
}

#[doc(hidden)]
impl<D, I> Address<Sensor<D, I>>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>> + 'static,
    I: WriteRead + Read + Write,
{
    pub fn signal_data_ready(&self) {
        self.notify(DataReady)
    }
}
