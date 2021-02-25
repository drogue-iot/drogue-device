mod parser;
mod ready;
mod socket_pool;

use socket_pool::SocketPool;

use crate::api::arbitrator::BusArbitrator;
use crate::api::delayer::Delayer;
use crate::api::ip::tcp::{TcpError, TcpStack};
use crate::api::ip::{IpAddress, IpAddressV4, IpProtocol, SocketAddress};
use crate::api::spi::{ChipSelect, SpiBus, SpiError};
use crate::api::wifi::{Join, JoinError, WifiSupplicant};
use crate::domain::time::duration::Milliseconds;
use crate::driver::wifi::eswifi::parser::{
    CloseResponse, ConnectResponse, JoinResponse, ReadResponse, WriteResponse,
};
use crate::driver::wifi::eswifi::ready::{AwaitReady, QueryReady};
use crate::driver::wifi::eswifi::ready::{EsWifiReady, EsWifiReadyPin};
use crate::hal::gpio::InterruptPin;
use crate::prelude::*;
use core::fmt::Write;
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::{consts::*, ArrayLength, String};

pub struct Shared {
    socket_pool: SocketPool,
}

impl Shared {
    fn new() -> Self {
        Self {
            socket_pool: SocketPool::new(),
        }
    }
}

pub struct Esp8266Wifi<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    shared: Shared,
    controller: ActorContext<Esp8266WifiController<UART, T, ENABLE, RESET>>,
    // ingress: ActorContext<Esp8266WifiIngress<UART, T>>,
}

impl<UART, T, ENABLE, RESET> Esp8266Wifi<UART, T, ENALBE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    #[allow(non_camel_case_types)]
    pub fn new(
        enable: ENABLE,
        reset: RESET,
    ) -> Self {
        Self {
            shared: Shared::new(),
            controller: ActorContext::new(Esp2866WifiController::new(enable, reset))
                .with_name("esp8266-wifi-controller"),
            // ingress: ActorContext::new(Esp8266WifiIngress::new()).with_name("esp8266-wifi-ingress"),
        }
    }
}

impl<UART, T, ENABLE, RESET> Package for Esp8266Wifi<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Primary = Esp8266WifiController<UART, T, ENABLE, RESET>;
    type Configuration = (Address<UART>, Address<T>);

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        self.controller
            .mount((&self.shared, config.0, config.1), supervisor)
        //self.ingress .mount((&self.shared, config.0, config.1), supervisor)
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.controller.address()
    }
}

pub struct Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    shared: Option<&'static Shared>,
    address: Option<Address<Self>>,
    uart: Option<Address<UART>>,
    delayer: Option<Address<T>>,
    reset: RESET,
    wakeup: WAKEUP,
    state: State,
}

macro_rules! command {
    ($size:tt, $($arg:tt)*) => ({
        //let mut c = String::new();
        //c
        let mut c = String::<$size>::new();
        write!(c, $($arg)*).unwrap();
        c.push_str("\r").unwrap();
        c
    })
}

impl<UART, T, ENABLE, RESET> Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(enable: ENABLE, reset: RESET) -> Self {
        Self {
            address: None,
            uart: None,
            delayer: None,
            enable,
            reset,
            state: State::Uninitialized,
            shared: None,
        }
    }

    async fn initialize(&mut self) {
    let mut buffer: [u8; 1024] = [0; 1024];
    let mut pos = 0;

    const READY: [u8; 7] = *b"ready\r\n";

    let mut counter = 0;

    self.enable
        .set_high()
        .map_err(|_| AdapterError::UnableToInitialize)?;
    self.reset
        .set_high()
        .map_err(|_| AdapterError::UnableToInitialize)?;

    log::debug!("waiting for adapter to become ready");

        let rx_buf = [u8; 1];
    loop {
        let result = self.uart.unwrap().read(&rx_buf[..]).await;
        match result {
            Ok(c) => {
                buffer[pos] = rx_buf[0];
                pos += 1;
                if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                    log::debug!("adapter is ready");
                    disable_echo(&mut tx, &mut rx)?;
                    enable_mux(&mut tx, &mut rx)?;
                    set_recv_mode(&mut tx, &mut rx)?;
                }
            }
            Err(e) => {
                log::error!("Error initializing ESP8266 modem");
                break;
            }
        }
    }
    }

    async fn start(mut self) -> Self {
        log::info!("[{}] start", ActorInfo::name());
        self
    }
}

impl<UART, T, ENABLE, RESET> WifiSupplicant for Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    fn join(mut self, join_info: Join) -> Response<Self, Result<IpAddress, JoinError>> {
        Response::defer(async move {
            /*
            TODO

            let result = match join_info {
                Join::Open => self.join_open().await,
                Join::Wpa { ssid, password } => {
                    self.join_wep(ssid.as_ref(), password.as_ref()).await
                }
            };*/

            (self, Err(JoinError::Unknown))
        })
    }
}

impl<UART, T, ENABLE, RESET> TcpStack for Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type SocketHandle = u8;

    fn open(self) -> Response<Self, Self::SocketHandle> {
        let open_future = self.shared.unwrap().socket_pool.open();
        Response::immediate_future(self, open_future)
    }

    fn connect(
        mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Response<Self, Result<(), TcpError>> {
        Response::defer(async move {
            (self, Err(TcpError::ConnectError))
        })
    }

    fn write(
        mut self,
        handle: Self::SocketHandle,
        buf: &[u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        Response::immediate(self, Err(TcpError::WriteError))
    }

    fn read(
        mut self,
        handle: Self::SocketHandle,
        buf: &mut [u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        Response::immediate(self, Err(TcpError::ReadError))
    }

    fn close(mut self, handle: Self::SocketHandle) -> Completion<Self> {
        Completion::immediate(self)
    }
}

impl<UART, T, ENABLE, RESET> Actor for Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: Uart + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Configuration = (
        &'static Shared,
        Address<UART>,
        Address<T>,
    );

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config.0);
        self.address.replace(address);
        self.uart.replace(config.1);
        self.delayer.replace(config.2);
    }

    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(self.start())
    }
}
