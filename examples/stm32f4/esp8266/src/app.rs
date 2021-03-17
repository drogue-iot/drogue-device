use core::cell::RefCell;
use core::str::FromStr;
use drogue_device::{
    api::{
        ip::{
            tcp::{TcpSocket, TcpStack},
            IpAddress, IpProtocol, SocketAddress,
        },
        wifi::{Join, WifiSupplicant},
    },
    prelude::*,
};

pub struct App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    driver: Option<Address<NET>>,
    socket: Option<RefCell<TcpSocket<NET>>>,
    ssid: &'static str,
    psk: &'static str,
    ip: IpAddress,
    port: u16,
}

impl<NET> App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    pub fn new(ssid: &'static str, psk: &'static str, ip: IpAddress, port: u16) -> Self {
        Self {
            driver: None,
            socket: None,
            ssid,
            psk,
            ip,
            port,
        }
    }
}

impl<NET> Actor for App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    type Configuration = Address<NET>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        log::info!("Bound wifi");
        self.driver.replace(config);
    }
    fn on_start(mut self) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().expect("driver not bound!");
            log::info!("Joining network");
            let ip = driver
                .wifi_join(Join::Wpa {
                    ssid: heapless::String::from_str(self.ssid).unwrap(),
                    password: heapless::String::from_str(self.psk).unwrap(),
                })
                .await
                .expect("Error joining wifi");
            log::info!("Joined wifi network with IP: {}", ip);
            let mut socket = driver.tcp_open().await;
            log::info!("Socket created");
            let result = socket
                .connect(IpProtocol::Tcp, SocketAddress::new(self.ip, self.port))
                .await;
            match result {
                Ok(_) => {
                    log::info!("Connected!");
                    self.socket.replace(RefCell::new(socket));
                }
                Err(e) => {
                    log::info!("Error connecting to host: {:?}", e);
                }
            }
            self
        })
    }
}
