use crate::domain::temperature::Celsius;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::driver::sensor::hts221::SensorAcquisition;
use crate::hal::gpio::InterruptPin;
use crate::handler::EventHandler;
use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct DataReady;

pub struct Ready<D, P, I>
where
    D: Device + 'static,
    P: InputPin + InterruptPin + 'static,
    I: WriteRead + Read + Write + 'static,
{
    pin: P,
    sensor: Option<Address<Sensor<D, I>>>,
}

impl<D, P, I> Ready<D, P, I>
where
    D: Device,
    P: InputPin + InterruptPin,
    I: WriteRead + Read + Write,
{
    pub fn new(pin: P) -> Self {
        Self { pin, sensor: None }
    }
}

impl<D, P, I> Actor for Ready<D, P, I>
where
    D: Device,
    P: InputPin + InterruptPin,
    I: WriteRead + Read + Write + 'static,
{
    type Configuration = Address<Sensor<D, I>>;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.sensor.replace(config);
    }
}

impl<D, P, I> Interrupt for Ready<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>> + 'static,
    P: InputPin + InterruptPin,
    I: WriteRead + Read + Write + 'static,
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            if let Some(sensor) = self.sensor {
                sensor.signal_data_ready()
            }
            self.pin.clear_interrupt();
        }
    }
}
