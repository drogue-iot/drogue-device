use core::str::from_utf8;
use drogue_device::api::ip::tcp::TcpStack;
use drogue_device::api::ip::{IpAddress, IpProtocol, SocketAddress};
use drogue_device::api::wifi::{Join, WifiSupplicant};
use drogue_device::domain::temperature::Celsius;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::driver::tls::handshake::ClientHello;
use drogue_device::prelude::*;
use rand_core::{CryptoRng, Error, RngCore};
use stm32l4xx_hal::rng::Rng;

pub struct Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    wifi: Option<Address<S>>,
    rng: Rng,
}
impl<S> Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    pub fn new(rng: Rng) -> Self {
        Self { wifi: None, rng }
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
                            SocketAddress::new(IpAddress::new_v4(192, 168, 1, 220), 8443),
                        )
                        .await;

                    match result {
                        Ok(_) => {
                            log::info!("connected to ssl server");
                            //let result = socket.write( b"GET / HTTP/1.1\r\nhost:192.168.1.8\r\n\r\n" ).await;
                            let random: [u8; 32] = [
                                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
                                20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
                            ];
                            let rng = RngImpl::new(&self.rng);
                            let client_hello = ClientHello::new(rng, random);
                            let result = client_hello.transmit(&mut socket).await;
                            match result {
                                Ok(_) => {
                                    log::info!("wrote HTTP request");
                                    loop {
                                        let mut buf = [0; 1024];
                                        let result = socket.read(&mut buf).await;
                                        match result {
                                            Ok(size) => {
                                                log::info!("received {}", size);
                                                log::info!("{:x?}", &buf[0..size]);
                                            }
                                            Err(_) => {
                                                log::info!("read error");
                                                break
                                            }
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

impl<S> NotifyHandler<SensorAcquisition<Celsius>> for Logic<S>
where
    S: WifiSupplicant + TcpStack + 'static,
{
    fn on_notify(self, message: SensorAcquisition<Celsius>) -> Completion<Self> {
        //unimplemented!()
        Completion::immediate(self)
    }
}

struct RngImpl<'a> {
    rng: &'a Rng,
}

impl Copy for RngImpl<'_> {}

impl Clone for RngImpl<'_> {
    fn clone(&self) -> Self {
        Self { rng: self.rng }
    }
}

impl<'a> RngImpl<'a> {
    pub fn new(rng: &'a Rng) -> Self {
        Self { rng }
    }
}

impl CryptoRng for RngImpl<'_> {}

impl RngCore for RngImpl<'_> {
    fn next_u32(&mut self) -> u32 {
        self.rng.get_random_data()
    }

    fn next_u64(&mut self) -> u64 {
        let a = self.rng.get_random_data();
        let b = self.rng.get_random_data();
        (a as u64) << 32 + b
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut data = 0;

        for (index, slot) in dest.iter_mut().enumerate() {
            if index % 4 == 0 {
                data = self.next_u32();
            }

            *slot = data as u8 & 0xff;
            data = data >> 8;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
