use core::cell::RefCell;
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
}

impl<NET> App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    pub fn new() -> Self {
        Self {
            driver: None,
            socket: None,
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
}

impl<NET> NotifyHandler<Join> for App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    fn on_notify(mut self, message: Join) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().expect("driver not bound!");
            log::info!("Joining network");
            let ip = driver.wifi_join(message).await.expect("Error joining wifi");
            log::info!("Joined wifi network with IP: {}", ip);
            let mut socket = driver.tcp_open().await;
            log::info!("Socket created");
            let result = socket
                .connect(
                    IpProtocol::Tcp,
                    SocketAddress::new(IpAddress::new_v4(192, 168, 1, 2), 12345),
                )
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

pub struct TakeMeasurement;

impl<NET> NotifyHandler<TakeMeasurement> for App<NET>
where
    NET: WifiSupplicant + TcpStack + 'static,
{
    fn on_notify(self, _: TakeMeasurement) -> Completion<Self> {
        Completion::defer(async move {
            {
                log::info!("Sending data");
                let mut socket = self
                    .socket
                    .as_ref()
                    .expect("socket not bound!")
                    .borrow_mut();
                log::info!("Writing data to socket");
                let result = socket.write(b"{\"temp\": 24.3}\r\n").await;
                match result {
                    Ok(_) => {
                        log::info!("Data sent");
                        let mut rx_buf = [0; 8];
                        loop {
                            let result = socket.read(&mut rx_buf[..]).await;
                            match result {
                                Ok(len) if &rx_buf[0..len] == b"OK\r\n" => {
                                    log::info!("Measurement confirmed");
                                    break;
                                }
                                Ok(len) if &rx_buf[0..len] == b"ERROR\r\n" => {
                                    log::info!("Error reporting measurement");
                                    break;
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    log::warn!("Error reading response: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Error sending measurement: {:?}", e);
                    }
                }
            }
            self
        })
    }
}
