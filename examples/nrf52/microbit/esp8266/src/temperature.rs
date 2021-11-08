use super::{App, AppSocket, Command, SensorData};
use drogue_device::{
    domain::{temperature::*, *},
    Actor, Address, Inbox,
};
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
    type Configuration = Address<'static, App<AppSocket>>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where M: 'm, 'd: 'm = impl core::future::Future<Output = ()> + 'm;

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
                    temperature: Temperature::<Celsius>::new(t.to_num::<f32>()),
                    relative_humidity: 0.0,
                };
                app.request(Command::Update(SensorData {
                    data: d,
                    location: None,
                }))
                .unwrap()
                .await;
                Timer::after(self.interval).await;
            }
        }
    }
}
