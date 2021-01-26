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
use crate::driver::sensor::hts221::ready::Ready;

pub struct Hts221<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    sensor: ActorContext<Sensor<I>>,
    ready: InterruptContext<Ready<P, I>>,
    sensor_addr: Option<Address<Sensor<I>>>,
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Hts221<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new<N: Nr>(ready: P, irq: N) -> Self {
        Self {
            sensor: ActorContext::new(Sensor::new()),
            ready: InterruptContext::new(Ready::new(ready), irq),
            sensor_addr: None,
        }
    }

    pub fn mount(&'static mut self, supervisor: &mut Supervisor) -> Address<Sensor<I>> {
        let ready_addr = self.ready.start(supervisor);
        let sensor_addr = self.sensor.mount(supervisor);
        ready_addr.bind(&sensor_addr);
        self.sensor_addr.replace(sensor_addr.clone());
        sensor_addr
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<Mutex<I>> for Hts221<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Mutex<I>>) {
        self.sensor_addr.as_ref().unwrap().bind(&address);
    }
}

