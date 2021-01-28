use crate::bind::Bind;
use crate::driver::sensor::hts221::ready::Ready;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::ops::Add;
use cortex_m::interrupt::Nr;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct Hts221<D: Device + 'static, P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
{
    sensor: ActorContext<D, Sensor<D, I>>,
    ready: InterruptContext<D, Ready<D, P, I>>,
}

impl<D: Device, P: InputPin + ExtiPin, I: WriteRead + Read + Write> Hts221<D, P, I> {
    pub fn new<N: Nr>(ready: P, irq: N) -> Self {
        Self {
            sensor: ActorContext::new(Sensor::new()),
            ready: InterruptContext::new(Ready::new(ready), irq),
        }
    }

    pub fn mount(
        &'static self,
        bus: &EventBus<D>,
        supervisor: &mut Supervisor,
    ) -> Address<D, Sensor<D, I>> {
        let ready_addr = self.ready.mount(bus, supervisor);
        let sensor_addr = self.sensor.mount(bus, supervisor);
        ready_addr.bind(&sensor_addr);
        sensor_addr
    }
}
