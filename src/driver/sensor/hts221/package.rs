use crate::driver::sensor::hts221::ready::Ready;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::driver::sensor::hts221::SensorAcquisition;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::handler::EventHandler;
use crate::package::Package;
use crate::prelude::*;
use cortex_m::interrupt::Nr;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;
use crate::domain::temperature::Celsius;
use crate::driver::i2c::I2cPeripheral;

pub struct Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>> + 'static,
    P: InputPin + ExtiPin + 'static,
    I: WriteRead + Read + Write + 'static,
{
    sensor: ActorContext<Sensor<D, I>>,
    ready: InterruptContext<Ready<D, P, I>>,
}

impl<D, P, I> Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>>,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write,
{
    pub fn new<N: Nr>(ready: P, irq: N) -> Self {
        Self {
            sensor: ActorContext::new(Sensor::new()).with_name("hts221-sensor"),
            ready: InterruptContext::new(Ready::new(ready), irq).with_name("hts221-irq"),
        }
    }

    pub fn bind(&'static self, address: Address<I2cPeripheral<I>>) {
        self.sensor.bind(address);
    }
}

impl<D, P, I> Package<D, Sensor<D, I>> for Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>>,
    P: InputPin + ExtiPin,
    I: WriteRead + Read + Write,
{
    fn mount(
        &'static self,
        bus_address: Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<Sensor<D, I>> {
        let ready_addr = self.ready.mount(supervisor);
        let sensor_addr = self.sensor.mount(supervisor);
        self.sensor.bind(bus_address);
        self.ready.bind(sensor_addr);
        sensor_addr
    }
}
