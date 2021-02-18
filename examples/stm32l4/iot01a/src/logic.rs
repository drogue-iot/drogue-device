use drogue_device::api::wifi::{Join, WifiSupplicant};
use drogue_device::prelude::*;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::domain::temperature::Celsius;

pub struct Logic<S>
where
    S: WifiSupplicant + 'static,
{
    wifi: Option<Address<S>>,
}
impl<S> Logic<S>
    where
        S: WifiSupplicant + 'static,
{
    pub fn new() -> Self {
        Self {
            wifi: None,
        }
    }
}

impl<S> Actor for Logic<S>
where
    S: WifiSupplicant + 'static,
{
    type Configuration = Address<S>;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.wifi.replace(config);
    }

    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(async move {
            let result = self.wifi.unwrap().wifi_join(Join::Wpa {
                ssid: "drogue".into(),
                password: "rodneygnome".into(),
            }).await;
            self
        })
    }
}

impl<S> NotifyHandler<SensorAcquisition<Celsius>> for Logic<S>
    where
        S: WifiSupplicant + 'static,
{
    fn on_notify(self, message: SensorAcquisition<Celsius>) -> Completion<Self> {
        //unimplemented!()
        Completion::immediate(self)
    }
}
