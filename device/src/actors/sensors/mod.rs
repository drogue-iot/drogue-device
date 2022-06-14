use crate::domain::{temperature::TemperatureScale, SensorAcquisition};
use crate::traits::sensors::temperature::TemperatureSensor;
use core::future::Future;
use ector::{Actor, Address, Inbox};
use embedded_hal::digital::v2::InputPin;
use embedded_hal_async::digital::Wait;

pub struct Temperature<P, T, C>
where
    P: Wait + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    ready: P,
    sensor: T,
    dest: Address<SensorAcquisition<C>>,
}

impl<P, T, C> Temperature<P, T, C>
where
    P: Wait + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    pub fn new(ready: P, sensor: T, dest: Address<SensorAcquisition<C>>) -> Self {
        Self {
            sensor,
            ready,
            dest,
        }
    }

    async fn wait_ready(&mut self) {
        while !self.ready.is_high().ok().unwrap() {
            self.ready.wait_for_any_edge().await.unwrap();
        }
    }
}

impl<P, T, C> Actor for Temperature<P, T, C>
where
    P: Wait + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    type Message<'m> = ();
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where Self: 'm, M: 'm + Inbox<Self::Message<'m>>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        _: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            let _ = self.sensor.calibrate().await;
            loop {
                self.wait_ready().await;
                let data = self.sensor.temperature().await;
                if let Ok(data) = data {
                    self.dest.notify(data).await;
                }
            }
        }
    }
}
