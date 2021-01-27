use crate::bind::Bind;
use crate::driver::sensor::hts221::ready::Ready;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::fmt::Debug;
use core::ops::Add;
use cortex_m::interrupt::Nr;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct Hts221<D: Device + 'static, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
where
    <I as WriteRead>::Error: Debug,
    <I as Write>::Error: Debug,
{
    sensor: ActorContext<D, Sensor<D, I>>,
    ready: InterruptContext<D, Ready<D, P, I>>,
    sensor_addr: Option<Address<D, Sensor<D, I>>>,
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write> Hts221<D, P, I>
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

    pub fn mount(
        &'static mut self,
        device: &'static mut D,
        supervisor: &mut Supervisor,
    ) -> Address<D, Sensor<D, I>> {
        let ready_addr = self.ready.start(device, supervisor);
        let sensor_addr = self.sensor.mount(device, supervisor);
        ready_addr.bind(&sensor_addr);
        self.sensor_addr.replace(sensor_addr.clone());
        sensor_addr
    }
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<D, Mutex<D, I>>
    for Hts221<D, P, I>
where
    <I as WriteRead>::Error: Debug,
    <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<D, Mutex<D, I>>) {
        self.sensor_addr.as_ref().unwrap().bind(&address);
    }
}
