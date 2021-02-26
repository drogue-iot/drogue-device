mod num;
mod parser;
mod protocol;
mod socket_pool;

use socket_pool::SocketPool;

use crate::api::delayer::Delayer;
use crate::api::ip::tcp::{TcpError, TcpStack};
use crate::api::ip::{IpAddress, IpAddressV4, IpProtocol, SocketAddress};
use crate::api::uart::{Error as UartError, UartReader, UartWriter};
use crate::api::wifi::{Join, JoinError, WifiSupplicant};
use crate::domain::time::duration::Milliseconds;
use crate::hal::gpio::InterruptPin;
use crate::prelude::*;
use core::fmt::Write;
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::{consts::*, ArrayLength, String};

pub const BUFFER_LEN: usize = 1024;

#[derive(Debug)]
pub enum AdapterError {
    UnableToInitialize,
    NoAvailableSockets,
    Timeout,
    UnableToOpen,
    UnableToClose,
    WriteError,
    ReadError,
    InvalidSocket,
}

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

enum State {
    Uninitialized,
    Ready,
}

pub struct Esp8266Wifi<UART, T, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    shared: Shared,
    controller: ActorContext<Esp8266WifiController<UART, T, ENABLE, RESET>>,
    // ingress: ActorContext<Esp8266WifiIngress<UART, T>>,
}

impl<UART, T, ENABLE, RESET> Esp8266Wifi<UART, T, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    #[allow(non_camel_case_types)]
    pub fn new(enable: ENABLE, reset: RESET) -> Self {
        Self {
            shared: Shared::new(),
            controller: ActorContext::new(Esp8266WifiController::new(enable, reset))
                .with_name("esp8266-wifi-controller"),
            // ingress: ActorContext::new(Esp8266WifiIngress::new()).with_name("esp8266-wifi-ingress"),
        }
    }
}

impl<UART, T, ENABLE, RESET> Package for Esp8266Wifi<UART, T, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
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
    UART: UartWriter + UartReader + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    shared: Option<&'static Shared>,
    address: Option<Address<Self>>,
    uart: Option<Address<UART>>,
    delayer: Option<Address<T>>,
    enable: ENABLE,
    reset: RESET,
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
    UART: UartWriter + UartReader + 'static,
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

    async fn initialize(mut self) -> Self {
        let mut buffer: [u8; 1024] = [0; 1024];
        let mut pos = 0;

        const READY: [u8; 7] = *b"ready\r\n";

        let mut counter = 0;

        self.enable.set_high().ok().unwrap();
        self.reset.set_high().ok().unwrap();

        log::info!("waiting for adapter to become ready");

        let mut rx_buf: [u8; 1] = [0; 1];
        loop {
            let result = self.uart.unwrap().read(&mut rx_buf[..]).await;
            match result {
                Ok(c) => {
                    buffer[pos] = rx_buf[0];
                    pos += 1;
                    if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                        log::info!("adapter is ready");
                        self.disable_echo()
                            .await
                            .map_err(|e| log::error!("Error disabling echo mode"));
                        log::info!("Echo disabled");
                        self.enable_mux()
                            .await
                            .map_err(|e| log::error!("Error enabling mux"));
                        log::info!("Mux enabled");
                        self.set_recv_mode()
                            .await
                            .map_err(|e| log::error!("Error setting receive mode"));
                        log::info!("adapter configured");
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Error initializing ESP8266 modem");
                    break;
                }
            }
        }
        self
    }

    async fn start(mut self) -> Self {
        log::info!("[{}] start", ActorInfo::name());
        self
    }

    async fn write_command(&self, cmd: &[u8]) -> Result<(), UartError> {
        self.uart.as_ref().unwrap().write(cmd).await
    }

    async fn disable_echo(&self) -> Result<(), AdapterError> {
        self.write_command(b"ATE0\r\n")
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?;
        Ok(self
            .wait_for_ok()
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?)
    }

    async fn enable_mux(&self) -> Result<(), AdapterError> {
        self.write_command(b"AT+CIPMUX=1\r\n")
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?;
        Ok(self
            .wait_for_ok()
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?)
    }

    async fn set_recv_mode(&self) -> Result<(), AdapterError> {
        self.write_command(b"AT+CIPRECVMODE=1\r\n")
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?;
        Ok(self
            .wait_for_ok()
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?)
    }

    async fn wait_for_ok(&self) -> Result<(), AdapterError> {
        let mut buf: [u8; 64] = [0; 64];
        let mut pos = 0;

        loop {
            self.uart
                .as_ref()
                .unwrap()
                .read(&mut buf[pos..pos + 1])
                .await
                .map_err(|_| AdapterError::ReadError)?;
            pos += 1;
            if buf[0..pos].ends_with(b"OK\r\n") {
                return Ok(());
            } else if buf[0..pos].ends_with(b"ERROR\r\n") {
                return Err(AdapterError::UnableToInitialize);
            }
        }
    }
}

impl<UART, T, ENABLE, RESET> WifiSupplicant for Esp8266WifiController<UART, T, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
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
    UART: UartWriter + UartReader + 'static,
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
        Response::defer(async move { (self, Err(TcpError::ConnectError)) })
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
    UART: UartWriter + UartReader + 'static,
    T: Delayer + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Configuration = (&'static Shared, Address<UART>, Address<T>);

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config.0);
        self.address.replace(address);
        self.uart.replace(config.1);
        self.delayer.replace(config.2);
    }

    fn on_initialize(mut self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(self.initialize())
    }

    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(self.start())
    }
}
