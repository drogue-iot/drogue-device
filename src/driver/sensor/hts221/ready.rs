use crate::bind::Bind;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::ops::Add;
use cortex_m::interrupt::Nr;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct DataReady;

pub struct Ready<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
{
    pin: P,
    sensor: Option<Address<D, Sensor<D, I>>>,
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write> Ready<D, P, I>
{
    pub fn new(pin: P) -> Self {
        Self { pin, sensor: None }
    }
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> Actor<D>
    for Ready<D, P, I>
{
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
    NotificationHandler<Lifecycle> for Ready<D, P, I>
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D: Device + 'static, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> Interrupt<D>
    for Ready<D, P, I>
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

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<D, Sensor<D, I>>
    for Ready<D, P, I>
{
    fn on_bind(&'static mut self, address: Address<D, Sensor<D, I>>) {
        self.sensor.replace(address);
    }
}
