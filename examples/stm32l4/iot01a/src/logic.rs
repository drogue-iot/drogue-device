use core::str::from_utf8;
use drogue_device::api::ip::tcp::TcpStack;
use drogue_device::api::ip::{IpAddress, IpProtocol, SocketAddress};
use drogue_device::api::wifi::{Join, WifiSupplicant};
use drogue_device::domain::temperature::Celsius;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::driver::tls::config::Config;
use drogue_device::platform::cortex_m::stm32l4xx::rng::Random;
use drogue_device::prelude::*;
use stm32l4xx_hal::rng::Rng as HalRng;

pub struct Logic<W, T>
where
    W: WifiSupplicant + 'static,
    T: TcpStack + 'static,
{
    wifi: Option<Address<W>>,
    tcp: Option<Address<T>>,
}
impl<W, T> Logic<W, T>
where
    W: WifiSupplicant + 'static,
    T: TcpStack + 'static,
{
    pub fn new() -> Self {
        Self {
            wifi: None,
            tcp: None,
        }
    }
}

impl<W, T> Actor for Logic<W, T>
where
    W: WifiSupplicant + 'static,
    T: TcpStack + 'static,
{
    type Configuration = (Address<W>, Address<T>);

    fn on_mount(&mut self, _address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.wifi.replace(config.0);
        self.tcp.replace(config.1);
    }

    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(async move {
            let result = self
                .wifi
                .unwrap()
                .wifi_join(Join::Wpa {
                    ssid: "drogue".into(),
                    password: "rodneygnome".into(),
                })
                .await;

            match result {
                Ok(_) => {
                    log::info!("connected to wifi");

                    let mut socket = self.tcp.unwrap().tcp_open().await;
                    log::info!("got socket");
                    let result = socket
                        .connect(
                            IpProtocol::Tcp,
                            SocketAddress::new(IpAddress::new_v4(192, 168, 1, 220), 8443),
                        )
                        .await;

                    match result {
                        Ok(_) => {
                            log::info!("connected to TLS server");
                        }
                        Err(_) => {
                            log::info!("unable to connect TLS server");
                        }
                    }
                }
                Err(_) => {
                    log::info!("not connected to wifi");
                }
            }

            self
        })
    }
}

impl<W, T> NotifyHandler<SensorAcquisition<Celsius>> for Logic<W, T>
where
    W: WifiSupplicant + 'static,
    T: TcpStack + 'static,
{
    fn on_notify(self, message: SensorAcquisition<Celsius>) -> Completion<Self> {
        //unimplemented!()
        Completion::immediate(self)
    }
}
