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
    driver::{
        memory::{Memory, Query},
        timer::*,
        uart::dma::DmaUart,
        wifi::esp8266::Esp8266Wifi,
    },
    platform::cortex_m::nrf::{gpiote::*, timer::Timer as HalTimer, uarte::Uarte as HalUart},
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;

use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart =
    DmaUart<HalUart<UARTE0>, <AppTimer as Package>::Primary, consts::U64, consts::U1024>;
pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type Wifi =
    Esp8266Wifi<<AppUart as Package>::Primary, Pin<Output<PushPull>>, Pin<Output<PushPull>>>;
pub type AppWifi = <Wifi as Package>::Primary;

pub struct MyDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<Button>,
    pub btn_send: ActorContext<Button>,
    pub memory: ActorContext<Memory>,
    pub uart: AppUart,
    pub timer: AppTimer,
    pub wifi: Wifi,
    pub app: ActorContext<App<AppWifi>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_connect.mount(config.event_bus, supervisor);
        self.btn_send.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let wifi = self.wifi.mount(uart, supervisor);
        self.app.mount(wifi, supervisor);
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.btn_send.address().notify(event);
        self.btn_connect.address().notify(event);
    }
}

impl EventHandler<PinEvent> for MyDevice {
    fn on_event(&'static self, event: PinEvent) {
        match event {
            PinEvent(Channel::Channel0, PinState::Low) => {
                self.app.address().notify(Join::Wpa {
                    ssid: heapless::String::from_str("foo").unwrap(),
                    password: heapless::String::from_str("bar").unwrap(),
                });
                self.memory.address().notify(Query);
            }
            PinEvent(Channel::Channel1, PinState::Low) => {
                self.app.address().notify(TakeMeasurement);
                self.memory.address().notify(Query);
            }
            _ => {}
        }
    }
}

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
