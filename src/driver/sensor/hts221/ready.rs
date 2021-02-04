use crate::bind::Bind;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::driver::sensor::hts221::SensorAcquisition;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::handler::EventHandler;
use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct DataReady;

pub struct Ready<D, P, I>
where
    D: Device + 'static,
    P: InputPin + ExtiPin + 'static,
    I: WriteRead + Read + Write + 'static,
{
    pin: P,
    sensor: Option<Address<Sensor<D, I>>>,
}

impl<D, P, I> Ready<D, P, I>
where
    D: Device,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write,
{
    pub fn new(pin: P) -> Self {
        Self { pin, sensor: None }
    }
}

impl<D, P, I> Actor for Ready<D, P, I>
where
    D: Device,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write + 'static,
{
}

impl<D, P, I> Interrupt for Ready<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition> + 'static,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write + 'static,
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            log::trace!("[hts221] READY");
            if let Some(sensor) = self.sensor.as_ref() {
                sensor.signal_data_ready()
            }
            self.pin.clear_interrupt_pending_bit();
        }
    }
}

impl<D, P, I> Bind<Sensor<D, I>> for Ready<D, P, I>
where
    D: Device,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write,
{
    fn on_bind(&mut self, address: Address<Sensor<D, I>>) {
        self.sensor.replace(address);
    }
}
