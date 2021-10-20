use crate::domain::temperature::Celsius;
use crate::drivers::sensors::hts221::*;

use crate::{Actor, Address, Inbox};
use core::future::Future;
use core::marker::PhantomData;
use embassy::traits::gpio::WaitForAnyEdge;
use embassy::traits::i2c::*;

use crate::domain::temperature::TemperatureScale;

pub trait SensorMonitor<S: TemperatureScale> {
    fn notify(&self, value: SensorAcquisition<S>);
}

pub struct Sensor<P, I, S>
where
    P: WaitForAnyEdge + 'static,
    I: I2c<SevenBitAddress> + 'static,
    S: SensorMonitor<Celsius> + 'static,
{
    hts221: Hts221,
    _data: PhantomData<(&'static I, &'static S)>,
    ready: P,
}

impl<P, I, S> Sensor<P, I, S>
where
    P: WaitForAnyEdge + 'static,
    I: I2c<SevenBitAddress> + 'static,
    S: SensorMonitor<Celsius> + 'static,
{
    pub fn new(ready: P) -> Self {
        Self {
            hts221: Hts221::new(),
            _data: PhantomData,
            ready,
        }
    }
}

impl<P, I, S> Actor for Sensor<P, I, S>
where
    P: WaitForAnyEdge + 'static,
    I: I2c<SevenBitAddress> + 'static,
    S: SensorMonitor<Celsius> + 'static,
{
    type Configuration = (I, S);

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm, I: 'm = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        let (mut i2c, monitor) = config;
        async move {
            self.hts221.initialize(&mut i2c).await.ok();
            loop {
                self.ready.wait_for_any_edge().await;
                let data = self.hts221.read(&mut i2c).await.ok().unwrap();
                monitor.notify(data);
            }
        }
    }
}
