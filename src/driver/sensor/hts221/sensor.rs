use crate::bind::Bind;
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
use crate::synchronization::Mutex;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

pub const ADDR: u8 = 0x5F;

pub struct Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write + 'static,
{
    address: I2cAddress,
    i2c: Option<Address<Mutex<I>>>,
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
    fn initialize(&'static mut self) -> Completion<Self> {
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;

                Ctrl2::modify(self.address, &mut i2c, |reg| {
                    reg.boot();
                });

                Ctrl1::modify(self.address, &mut i2c, |reg| {
                    reg.power_active()
                        .output_data_rate(OutputDataRate::Hz1)
                        .block_data_update(BlockDataUpdate::MsbLsbReading);
                });

                Ctrl3::modify(self.address, &mut i2c, |reg| {
                    reg.enable(true);
                });

                //log::info!(
                //"[hts221] address=0x{:X}",
                //WhoAmI::read(self.address, &mut i2c)
                //);

                //let result = self.timer.as_ref().unwrap().request( Delay( Milliseconds(85u32))).await;
                loop {
                    // Ensure status is emptied
                    if !Status::read(self.address, &mut i2c).any_available() {
                        break;
                    }
                    Hout::read(self.address, &mut i2c);
                    Tout::read(self.address, &mut i2c);
                }
            }
            (self)
        })
    }

    fn start(&'static mut self) -> Completion<Self> {
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;
                self.calibration
                    .replace(Calibration::read(self.address, &mut i2c));
            }
            (self)
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

impl<D, I> Bind<Mutex<I>> for Sensor<D, I>
where
    D: Device,
    I: WriteRead + Read + Write + 'static,
{
    fn on_bind(&mut self, address: Address<Mutex<I>>) {
        self.i2c.replace(address);
    }
}

impl<D, I> NotifyHandler<DataReady> for Sensor<D, I>
where
    D: Device + EventHandler<SensorAcquisition>,
    I: WriteRead + Read + Write,
{
    fn on_notify(&'static mut self, message: DataReady) -> Completion<Self> {
        Completion::defer(async move {
            if self.i2c.is_some() {
                let mut i2c = self.i2c.as_ref().unwrap().lock().await;

                if let Some(ref calibration) = self.calibration {
                    let t_out = Tout::read(self.address, &mut i2c);
                    let temperature = calibration.calibrated_temperature(t_out);

                    let h_out = Hout::read(self.address, &mut i2c);
                    let relative_humidity = calibration.calibrated_humidity(h_out);

                    self.bus.as_ref().unwrap().publish(SensorAcquisition {
                        temperature,
                        relative_humidity,
                    });
                //log::info!(
                //"[hts221] temperature={:.2}°F humidity={:.2}%rh",
                //temperature.into_fahrenheit(),
                //relative_humidity
                //);
                } else {
                    log::info!("[hts221] no calibration data available")
                }
            }
            (self)
        })
    }
}

#[doc(hidden)]
impl<D, I> Address<Sensor<D, I>>
where
    D: Device + EventHandler<SensorAcquisition> + 'static,
    I: WriteRead + Read + Write,
{
    pub fn signal_data_ready(&self) {
        self.notify(DataReady)
    }
}
