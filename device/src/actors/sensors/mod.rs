use crate::domain::{temperature::TemperatureScale, SensorAcquisition};
use crate::traits::sensors::temperature::TemperatureSensor;
use crate::{Actor, Address, Inbox};
use core::future::Future;
use embassy::traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub struct Temperature<P, T, C>
where
    P: WaitForAnyEdge + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    ready: P,
    sensor: T,
    _scale: core::marker::PhantomData<&'static C>,
}

impl<P, T, C> Temperature<P, T, C>
where
    P: WaitForAnyEdge + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    pub fn new(ready: P, sensor: T) -> Self {
        Self {
            sensor,
            ready,
            _scale: core::marker::PhantomData,
        }
    }

    async fn wait_ready(&mut self) {
        while !self.ready.is_high().ok().unwrap() {
            self.ready.wait_for_any_edge().await;
        }
    }
}

pub enum Command {
    ReadTemperature,
    Calibrate,
}

impl<P, T, C> Actor for Temperature<P, T, C>
where
    P: WaitForAnyEdge + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    type Message<'m> = Command;
    type Response = Option<Result<SensorAcquisition<C>, T::Error>>;

    type OnMountFuture<'m, M>
    where
        Self: 'm,
        M: 'm,
        P: 'm,
        T: 'm,
        C: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            let _ = self.sensor.calibrate().await;
            loop {
                if let Some(mut m) = inbox.next().await {
                    self.wait_ready().await;
                    let data = self.sensor.temperature().await;
                    m.set_response(Some(data));
                }
            }
        }
    }
}

impl<P, T, C> TemperatureSensor<C> for Address<'static, Temperature<P, T, C>>
where
    P: WaitForAnyEdge + InputPin + 'static,
    T: TemperatureSensor<C> + 'static,
    C: TemperatureScale + 'static,
{
    type Error = T::Error;

    type CalibrateFuture<'m>
    where
        P: 'm,
        T: 'm,
        C: 'm,
    = impl Future<Output = Result<(), Self::Error>> + 'm;

    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m> {
        async move {
            self.request(Command::Calibrate)
                .unwrap()
                .await
                .unwrap()
                .map(|_| ())
        }
    }

    type ReadFuture<'m>
    where
        P: 'm,
        T: 'm,
        C: 'm,
    = impl Future<Output = Result<SensorAcquisition<C>, Self::Error>> + 'm;

    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m> {
        async move {
            self.request(Command::ReadTemperature)
                .unwrap()
                .await
                .unwrap()
        }
    }
}
