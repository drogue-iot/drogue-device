mod buffer;
mod num;
mod parser;
mod protocol;
mod socket_pool;

use socket_pool::SocketPool;

use crate::api::ip::tcp::{TcpError, TcpStack};
use crate::api::ip::{IpAddress, IpAddressV4, IpProtocol, SocketAddress};
use crate::api::queue::*;
use crate::api::uart::{Error as UartError, UartReader, UartWriter};
use crate::api::wifi::{Join, JoinError, WifiSupplicant};
use crate::domain::time::duration::Milliseconds;
use crate::driver::queue::spsc_queue::*;
use crate::hal::gpio::InterruptPin;
use crate::prelude::*;
use buffer::Buffer;
use core::cell::{RefCell, UnsafeCell};
use core::fmt::Write;
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::{consts, String};
use protocol::{Command, ConnectionType, Response as AtResponse, WiFiMode};

type QueueActor = <SpscQueue<AtResponse, consts::U2> as Package>::Primary;
pub const BUFFER_LEN: usize = 512;

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
    OperationNotSupported,
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

pub struct Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    shared: Shared,
    controller: ActorContext<Esp8266WifiController<UART>>,
    ingress: ActorContext<Esp8266WifiModem<UART, ENABLE, RESET>>,
    response_queue: SpscQueue<AtResponse, consts::U2>,
    notification_queue: SpscQueue<AtResponse, consts::U2>,
}

impl<UART, ENABLE, RESET> Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    #[allow(non_camel_case_types)]
    pub fn new(enable: ENABLE, reset: RESET) -> Self {
        Self {
            shared: Shared::new(),
            controller: ActorContext::new(Esp8266WifiController::new())
                .with_name("esp8266-wifi-controller"),
            ingress: ActorContext::new(Esp8266WifiModem::new(enable, reset))
                .with_name("esp8266-wifi-ingress"),
            response_queue: SpscQueue::new(),
            notification_queue: SpscQueue::new(),
        }
    }
}

impl<UART, ENABLE, RESET> Package for Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: UartWriter + UartReader + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Primary = Esp8266WifiController<UART>;
    type Configuration = Address<UART>;

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let response_queue = self.response_queue.mount((), supervisor);
        let notification_queue = self.notification_queue.mount((), supervisor);
        let addr = self.controller.mount(
            (&self.shared, response_queue, notification_queue, config),
            supervisor,
        );
        self.ingress
            .mount((response_queue, notification_queue, config), supervisor);
        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.controller.address()
    }
}

pub struct Esp8266WifiController<UART>
where
    UART: UartWriter + UartReader + 'static,
{
    shared: Option<&'static Shared>,
    address: Option<Address<Self>>,
    uart: Option<Address<UART>>,
    state: State,
    response_queue: Option<Address<QueueActor>>,
    notification_queue: Option<Address<QueueActor>>,
}

impl<UART> Esp8266WifiController<UART>
where
    UART: UartWriter + UartReader + 'static,
{
    pub fn new() -> Self {
        Self {
            address: None,
            uart: None,
            state: State::Uninitialized,
            shared: None,
            response_queue: None,
            notification_queue: None,
        }
    }

    async fn send<'c>(&mut self, command: Command<'c>) -> Result<AtResponse, AdapterError> {
        let mut bytes = command.as_bytes();
        log::trace!(
            "writing command {}",
            core::str::from_utf8(bytes.as_bytes()).unwrap()
        );

        let uart = self.uart.as_ref().unwrap();
        uart.write(&bytes.as_bytes())
            .await
            .map_err(|e| AdapterError::WriteError)?;

        uart.write(b"\r\n")
            .await
            .map_err(|e| AdapterError::WriteError)?;

        self.wait_for_response().await
    }

    async fn wait_for_response(&mut self) -> Result<AtResponse, AdapterError> {
        self.response_queue
            .as_ref()
            .unwrap()
            .dequeue()
            .await
            .map_err(|_| AdapterError::ReadError)
    }

    async fn start(mut self) -> Self {
        log::info!("[{}] start", ActorInfo::name());
        self
    }

    async fn set_mode(&mut self, mode: WiFiMode) -> Result<(), ()> {
        let command = Command::SetMode(mode);
        match self.send(command).await {
            Ok(AtResponse::Ok) => Ok(()),
            _ => Err(()),
        }
    }

    async fn join_wep(&mut self, ssid: &str, password: &str) -> Result<IpAddress, JoinError> {
        let command = Command::JoinAp { ssid, password };
        match self.send(command).await {
            Ok(AtResponse::Ok) => self.get_ip_address().await.map_err(|_| JoinError::Unknown),
            Ok(AtResponse::WifiConnectionFailure(reason)) => {
                log::warn!("Error connecting to wifi: {:?}", reason);
                Err(JoinError::Unknown)
            }
            _ => Err(JoinError::UnableToAssociate),
        }
    }

    async fn get_ip_address(&mut self) -> Result<IpAddress, ()> {
        let command = Command::QueryIpAddress;

        if let Ok(AtResponse::IpAddresses(addresses)) = self.send(command).await {
            return Ok(IpAddress::V4(addresses.ip));
        }

        Err(())
    }

    async fn process_notifications(&mut self) {
        let shared = self.shared.as_ref().unwrap();
        while let Some(response) = self
            .notification_queue
            .as_ref()
            .unwrap()
            .try_dequeue()
            .await
        {
            match response {
                AtResponse::DataAvailable { link_id, len } => {
                    //  shared.socket_pool // [link_id].available += len;
                }
                AtResponse::Connect(_) => {}
                AtResponse::Closed(link_id) => {
                    shared.socket_pool.close(link_id as u8);
                }
                _ => { /* ignore */ }
            }
        }
    }
}

impl<UART> WifiSupplicant for Esp8266WifiController<UART>
where
    UART: UartWriter + UartReader + 'static,
{
    fn join(mut self, join_info: Join) -> Response<Self, Result<IpAddress, JoinError>> {
        Response::defer(async move {
            let result = match join_info {
                Join::Open => Err(JoinError::Unknown),
                Join::Wpa { ssid, password } => {
                    self.join_wep(ssid.as_ref(), password.as_ref()).await
                }
            };
            (self, result)
        })
    }
}

impl<UART> TcpStack for Esp8266WifiController<UART>
where
    UART: UartWriter + UartReader + 'static,
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
            let command = Command::StartConnection(handle as usize, ConnectionType::TCP, dst);
            if let Ok(AtResponse::Connect(..)) = self.send(command).await {
                (self, Ok(()))
            } else {
                (self, Err(TcpError::ConnectError))
            }
        })
    }

    fn write(
        mut self,
        handle: Self::SocketHandle,
        buf: &[u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        unsafe {
            Response::defer_unchecked(async move {
                self.process_notifications().await;
                if self.shared.as_ref().unwrap().socket_pool.is_closed(handle) {
                    log::info!("No?!");
                    return (self, Err(TcpError::SocketClosed));
                }
                let command = Command::Send {
                    link_id: handle as usize,
                    len: buf.len(),
                };

                log::info!("Sending data");
                let result = match self.send(command).await {
                    Ok(AtResponse::Ok) => {
                        match self.wait_for_response().await {
                            Ok(AtResponse::ReadyForData) => {
                                let uart = self.uart.as_ref().unwrap();
                                if let Ok(_) = uart.write(buf).await {
                                    let mut data_sent: Option<usize> = None;
                                    loop {
                                        match self.wait_for_response().await {
                                            Ok(AtResponse::ReceivedDataToSend(len)) => {
                                                data_sent.replace(len);
                                            }
                                            Ok(AtResponse::SendOk) => {
                                                break Ok(data_sent.unwrap_or_default())
                                            }
                                            _ => {
                                                break Err(TcpError::WriteError);
                                                // unknown response
                                            }
                                        }
                                    }
                                } else {
                                    Err(TcpError::WriteError)
                                }
                            }
                            Ok(r) => {
                                log::info!("Unexpected response: {:?}", r);
                                Err(TcpError::WriteError)
                            }
                            Err(_) => Err(TcpError::WriteError),
                        }
                    }
                    Ok(r) => {
                        log::info!("Unexpected response: {:?}", r);
                        Err(TcpError::WriteError)
                    }
                    Err(_) => Err(TcpError::WriteError),
                };
                (self, result)
            })
        }
    }

    fn read(
        mut self,
        handle: Self::SocketHandle,
        buf: &mut [u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        unsafe {
            Response::defer_unchecked(async move {
                let mut rp = 0;
                loop {
                    let result = async {
                        self.process_notifications().await;
                        if self.shared.as_ref().unwrap().socket_pool.is_closed(handle) {
                            return (Err(TcpError::SocketClosed));
                        }

                        let command = Command::Receive {
                            link_id: handle as usize,
                            len: core::cmp::min(buf.len() - rp, BUFFER_LEN),
                        };

                        match self.send(command).await {
                            Ok(AtResponse::DataReceived(inbound, len)) => {
                                for (i, b) in inbound[0..len].iter().enumerate() {
                                    buf[rp + i] = *b;
                                }
                                Ok(len)
                            }
                            Ok(AtResponse::Ok) => Ok(0),
                            _ => Err(TcpError::ReadError),
                        }
                    }
                    .await;

                    match result {
                        Ok(len) => {
                            rp += len;
                            if len == 0 || rp == buf.len() {
                                return (self, Ok(rp));
                            }
                        }
                        Err(e) => {
                            if rp == 0 {
                                return (self, Err(e));
                            } else {
                                return (self, Ok(rp));
                            }
                        }
                    }
                }
            })
        }
    }

    fn close(mut self, handle: Self::SocketHandle) -> Completion<Self> {
        Completion::defer(async move {
            let command = Command::CloseConnection(handle as usize);
            match self.send(command).await {
                Ok(AtResponse::Ok) | Ok(AtResponse::UnlinkFail) => {
                    let shared = self.shared.as_ref().unwrap();
                    shared.socket_pool.close(handle);
                }
                _ => {}
            }
            self
        })
    }
}

impl<UART> Actor for Esp8266WifiController<UART>
where
    UART: UartWriter + UartReader + 'static,
{
    type Configuration = (
        &'static Shared,
        Address<QueueActor>,
        Address<QueueActor>,
        Address<UART>,
    );

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config.0);
        self.address.replace(address);
        self.response_queue.replace(config.1);
        self.notification_queue.replace(config.2);
        self.uart.replace(config.3);
    }

    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(self.start())
    }
}

pub struct Esp8266WifiModem<UART, ENABLE, RESET>
where
    UART: UartReader + UartWriter + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    uart: Option<Address<UART>>,
    response_queue: Option<Address<QueueActor>>,
    notification_queue: Option<Address<QueueActor>>,
    parse_buffer: Buffer,
    enable: ENABLE,
    reset: RESET,
}

impl<UART, ENABLE, RESET> Esp8266WifiModem<UART, ENABLE, RESET>
where
    UART: UartReader + UartWriter + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(enable: ENABLE, reset: RESET) -> Self {
        Self {
            uart: None,
            parse_buffer: Buffer::new(),
            response_queue: None,
            notification_queue: None,
            enable,
            reset,
        }
    }

    async fn digest(&mut self) -> Result<(), AdapterError> {
        let result = self.parse_buffer.parse();

        if let Ok(response) = result {
            if !matches!(response, AtResponse::None) {
                log::trace!("--> {:?}", response);
            }
            match response {
                AtResponse::None => {}
                AtResponse::Ok
                | AtResponse::Error
                | AtResponse::FirmwareInfo(..)
                | AtResponse::Connect(..)
                | AtResponse::ReadyForData
                | AtResponse::ReceivedDataToSend(..)
                | AtResponse::DataReceived(..)
                | AtResponse::SendOk
                | AtResponse::SendFail
                | AtResponse::WifiConnectionFailure(..)
                | AtResponse::IpAddress(..)
                | AtResponse::Resolvers(..)
                | AtResponse::DnsFail
                | AtResponse::UnlinkFail
                | AtResponse::IpAddresses(..) => {
                    if let Err(response) = self
                        .response_queue
                        .as_ref()
                        .unwrap()
                        .enqueue(response)
                        .await
                    {
                        log::error!("failed to enqueue response {:?}", response);
                    }
                }
                AtResponse::Closed(..) | AtResponse::DataAvailable { .. } => {
                    if let Err(response) = self
                        .notification_queue
                        .as_ref()
                        .unwrap()
                        .enqueue(response)
                        .await
                    {
                        log::error!("failed to enqueue notification {:?}", response);
                    }
                }
                AtResponse::WifiConnected => {
                    log::info!("wifi connected");
                }
                AtResponse::WifiDisconnect => {
                    log::info!("wifi disconnect");
                }
                AtResponse::GotIp => {
                    log::info!("wifi got ip");
                }
            }
        }
        Ok(())
    }

    async fn process(&mut self) -> Result<(), AdapterError> {
        let uart = self.uart.as_ref().unwrap();

        let mut buf = [0; 1];

        let len = uart
            .read(&mut buf[..]) //, Milliseconds(5000))
            .await
            .map_err(|_| AdapterError::ReadError)?;
        for b in &buf[..len] {
            self.parse_buffer.write(*b).unwrap();
        }
        Ok(())
    }

    async fn start(mut self) -> Self {
        log::info!("Starting ESP8266 Modem");
        loop {
            if let Err(e) = self.process().await {
                log::error!("Error reading data: {:?}", e);
            }

            if let Err(e) = self.digest().await {
                log::error!("Error digesting data");
            }
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

        let mut rx_buf = [0; 1];
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
                        log::info!("Recv mode configured");
                        self.set_mode()
                            .await
                            .map_err(|e| log::error!("Error setting station mode"));
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

    async fn write_command(&self, cmd: &[u8]) -> Result<(), UartError> {
        self.uart.as_ref().unwrap().write(cmd).await
    }

    async fn set_mode(&self) -> Result<(), AdapterError> {
        self.write_command(b"AT+CWMODE_CUR=1\r\n")
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?;
        Ok(self
            .wait_for_ok()
            .await
            .map_err(|_| AdapterError::UnableToInitialize)?)
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

impl<UART, ENABLE, RESET> Actor for Esp8266WifiModem<UART, ENABLE, RESET>
where
    UART: UartReader + UartWriter + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Configuration = (Address<QueueActor>, Address<QueueActor>, Address<UART>);

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.response_queue.replace(config.0);
        self.notification_queue.replace(config.1);
        self.uart.replace(config.2);
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
