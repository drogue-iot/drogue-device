use core::str::from_utf8;
use drogue_device::api::ip::tcp::TcpStack;
use drogue_device::api::ip::{IpAddress, IpProtocol, SocketAddress};
use drogue_device::api::wifi::{Join, WifiSupplicant};
use drogue_device::domain::temperature::Celsius;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::prelude::*;

pub struct Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    wifi: Option<Address<S>>,
}
impl<S> Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    pub fn new() -> Self {
        Self { wifi: None }
    }
}

impl<S> Actor for Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    type Configuration = Address<S>;

    fn on_mount(&mut self, _address: Address<Self>, config: Self::Configuration)
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
                    let mut socket = self.wifi.unwrap().tcp_open().await;
                    log::info!("got socket");
                    let result = socket
                        .connect(
                            IpProtocol::Tcp,
                            SocketAddress::new(IpAddress::new_v4(192, 168, 1, 245), 80),
                        )
                        .await;

                    match result {
                        Ok(_) => {
                            log::info!("connected to platform 80");
                            let result = socket
                                .write(b"GET / HTTP/1.1\r\nhost:192.168.1.8\r\n\r\n")
                                .await;
                            match result {
                                Ok(_) => {
                                    log::info!("wrote HTTP request");
                                    let mut buf = [0; 1024];
                                    let result = socket.read(&mut buf).await;
                                    match result {
                                        Ok(size) => {
                                            log::info!("received {}", size);
                                            log::info!("{}", from_utf8(&buf[0..size]).unwrap());
                                        }
                                        Err(_) => {
                                            log::info!("read error");
                                        }
                                    }
                                }
                                Err(_) => {
                                    log::info!("failed to write HTTP request");
                                }
                            }
                        }
                        Err(_) => {
                            log::info!("unable to connect 80");
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

impl<S> RequestHandler<SensorAcquisition<Celsius>> for Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    type Response = ();
    fn on_request(self, message: SensorAcquisition<Celsius>) -> Response<Self, ()> {
        //unimplemented!()
        Response::immediate(self, ())
    }
}
