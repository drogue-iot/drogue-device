use crate::domain::temperature::Celsius;
use crate::driver::i2c::I2cPeripheral;
use crate::driver::sensor::hts221::ready::Ready;
use crate::driver::sensor::hts221::sensor::Sensor;
use crate::driver::sensor::hts221::SensorAcquisition;
use crate::hal::gpio::InterruptPin;
use crate::handler::EventHandler;
use crate::package::Package;
use crate::prelude::*;
use cortex_m::interrupt::Nr;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;

pub struct Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>> + 'static,
    P: InputPin + InterruptPin + 'static,
    I: WriteRead + Read + Write + 'static,
{
    sensor: ActorContext<Sensor<D, I>>,
    ready: InterruptContext<Ready<D, P, I>>,
}

impl<D, P, I> Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>>,
    P: InputPin + InterruptPin,
    I: WriteRead + Read + Write,
{
    pub fn new<N: Nr>(ready: P, irq: N) -> Self {
        Self {
            sensor: ActorContext::new(Sensor::new()).with_name("hts221-sensor"),
            ready: InterruptContext::new(Ready::new(ready), irq).with_name("hts221-irq"),
        }
    }

    //pub fn bind(&'static self, address: Address<I2cPeripheral<I>>) {
    //self.sensor.bind(address);
    //}
}

impl<D, P, I> Package for Hts221<D, P, I>
where
    D: Device + EventHandler<SensorAcquisition<Celsius>>,
    P: InputPin + InterruptPin,
    I: WriteRead + Read + Write,
{
    type Primary = Sensor<D, I>;
    type Configuration = (Address<EventBus<D>>, Address<I2cPeripheral<I>>);

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let sensor_addr = self.sensor.mount(config, supervisor);
        let ready_addr = self.ready.mount(sensor_addr, supervisor);
        sensor_addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.sensor.address()
    }
}
