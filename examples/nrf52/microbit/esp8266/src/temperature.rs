use super::ConnectionFactory;
use drogue_device::{
    domain::{temperature::*, *},
    Actor, Address, Inbox,
};
use drogue_temperature::{App, Command, TemperatureData};
use embassy::time::{Duration, Timer};
use embassy_nrf::temp::Temp;

pub struct TemperatureMonitor<'d> {
    t: Temp<'d>,
    interval: Duration,
}

impl<'d> TemperatureMonitor<'d> {
    pub fn new(t: Temp<'d>, interval: Duration) -> Self {
        Self { t, interval }
    }
}

impl<'d> Actor for TemperatureMonitor<'d> {
    type Configuration = Address<'static, App<ConnectionFactory>>;

    type OnMountFuture<'m, M>
    where
        M: 'm,
        'd: 'm,
    = impl core::future::Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        app: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                let t = self.t.read().await;
                let d = SensorAcquisition {
                    temperature: Temperature::<Celsius>::new(),
                    relative_humidity: 0.0,
                };
                app.request(Command::Send(TemperatureData {
                    goloc: None,
                    temp: Some(t.to_num::<f32>()),
                    hum: None,
                }))
                .unwrap()
                .await;
                Timer::after(self.interval).await;
            }
        }
    }
}
