use super::{App, AppSocket, Command};
use drogue_device::{Actor, Address, Inbox};
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
                app.request(Command::Update(t.to_num::<f32>()))
                    .unwrap()
                    .await;
                Timer::after(self.interval).await;
            }
        }
    }
}
