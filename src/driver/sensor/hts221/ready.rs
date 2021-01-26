use crate::bind::Bind;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::fmt::Debug;
use core::ops::Add;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;
use crate::hal::gpio::exti_pin::ExtiPin;
use cortex_m::interrupt::Nr;
use crate::driver::sensor::hts221::sensor::Sensor;

pub struct DataReady;

pub struct Ready<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pin: P,
    sensor: Option<Address<Sensor<I>>>,
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            sensor: None,
        }
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> Actor for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    type Event = ();
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> NotificationHandler<Lifecycle> for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> Interrupt for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
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

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<Sensor<I>> for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Sensor<I>>) {
        self.sensor.replace(address);
    }
}
