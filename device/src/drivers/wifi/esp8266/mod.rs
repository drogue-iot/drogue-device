//! Esp8266 Async Driver
//!
//! An async driver for the Esp8266 AT-command firmware. The driver implements the drogue-network APIs for
//! WifiSupplicant and TcpStack.

mod buffer;
mod num;
mod parser;
mod protocol;

use crate::drivers::common::socket_pool::SocketPool;

use crate::traits::{
    ip::{IpAddress, IpProtocol, SocketAddress},
    tcp::{TcpError, TcpStack},
    wifi::{Join, JoinError, WifiSupplicant},
};
use atomic_polyfill::{AtomicBool, Ordering};
use buffer::Buffer;
use core::future::Future;
use embassy::{
    blocking_mutex::raw::NoopRawMutex,
    channel::{Channel, Receiver, Sender, Signal},
};
use embedded_hal::digital::v2::OutputPin;
use embedded_hal_async::serial::{Read, Write};
use protocol::{Command, ConnectionType, Response as AtResponse};

pub const BUFFER_LEN: usize = 512;
type DriverMutex = NoopRawMutex;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DriverError {
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

pub struct Initialized {
    signal: Signal<Result<(), DriverError>>,
    initialized: AtomicBool,
}

impl Initialized {
    pub const fn new() -> Self {
        Self {
            signal: Signal::new(),
            initialized: AtomicBool::new(false),
        }
    }

    async fn wait(&self) -> Result<bool, DriverError> {
        if self.initialized.swap(true, Ordering::SeqCst) == false {
            self.signal.wait().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn signal(&self, result: Result<(), DriverError>) {
        self.signal.signal(result);
    }
}

pub struct Esp8266Controller<'a, TX>
where
    TX: Write,
{
    initialized: &'a Initialized,
    tx: TX,
    socket_pool: SocketPool,
    response_consumer: Receiver<'a, DriverMutex, AtResponse, 2>,
    notification_consumer: Receiver<'a, DriverMutex, AtResponse, 2>,
}

pub struct Esp8266Modem<'a, RX, ENABLE, RESET>
where
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    initialized: &'a Initialized,
    rx: RX,
    enable: ENABLE,
    reset: RESET,
    parse_buffer: Buffer,
    response_producer: Sender<'a, DriverMutex, AtResponse, 2>,
    notification_producer: Sender<'a, DriverMutex, AtResponse, 2>,
}

pub struct Esp8266Driver {
    initialized: Initialized,
    response_channel: Channel<DriverMutex, AtResponse, 2>,
    notification_channel: Channel<DriverMutex, AtResponse, 2>,
}

impl Esp8266Driver {
    pub const fn new() -> Self {
        Self {
            initialized: Initialized::new(),
            response_channel: Channel::new(),
            notification_channel: Channel::new(),
        }
    }

    pub fn initialize<'a, TX, RX, ENABLE, RESET>(
        &'a self,
        tx: TX,
        rx: RX,
        enable: ENABLE,
        reset: RESET,
    ) -> (
        Esp8266Controller<'a, TX>,
        Esp8266Modem<'a, RX, ENABLE, RESET>,
    )
    where
        TX: Write + 'static,
        RX: Read + 'static,
        ENABLE: OutputPin + 'static,
        RESET: OutputPin + 'static,
    {
        let modem = Esp8266Modem::new(
            &self.initialized,
            rx,
            enable,
            reset,
            self.response_channel.sender(),
            self.notification_channel.sender(),
        );
        let controller = Esp8266Controller::new(
            &self.initialized,
            tx,
            self.response_channel.receiver(),
            self.notification_channel.receiver(),
        );

        (controller, modem)
    }
}

unsafe impl Sync for Esp8266Driver {}

impl<'a, RX, ENABLE, RESET> Esp8266Modem<'a, RX, ENABLE, RESET>
where
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(
        initialized: &'a Initialized,
        rx: RX,
        enable: ENABLE,
        reset: RESET,
        response_producer: Sender<'a, DriverMutex, AtResponse, 2>,
        notification_producer: Sender<'a, DriverMutex, AtResponse, 2>,
    ) -> Self {
        Self {
            initialized,
            rx,
            enable,
            reset,
            parse_buffer: Buffer::new(),
            response_producer,
            notification_producer,
        }
    }

    /*
                            self.disable_echo().await?;
                            trace!("Echo disabled");
                            self.enable_mux().await?;
                            trace!("Mux enabled");
                            self.set_recv_mode().await?;
                            trace!("Recv mode configured");
                            self.set_mode().await?;
                            return Ok(());
    */

    async fn initialize(&mut self) -> Result<(), DriverError> {
        self.enable.set_high().ok().unwrap();
        self.reset.set_high().ok().unwrap();
        let mut buffer: [u8; 1024] = [0; 1024];
        let mut pos = 0;

        const READY: [u8; 7] = *b"ready\r\n";

        info!("Initializing ESP8266");

        self.enable.set_high().ok().unwrap();
        self.reset.set_high().ok().unwrap();

        let mut rx_buf = [0; 1];
        loop {
            let result = self.rx.read(&mut rx_buf[..]).await;
            match result {
                Ok(_) => {
                    buffer[pos] = rx_buf[0];
                    pos += 1;
                    if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                        info!("ESP8266 initialized");
                        return Ok(());
                    }
                }
                Err(_) => {
                    error!("Error initializing ESP8266 modem");
                    return Err(DriverError::UnableToInitialize);
                }
            }
        }
    }

    /// Run the processing loop until an error is encountered
    pub async fn run(&mut self) -> ! {
        let result = self.initialize().await;
        self.initialized.signal(result);
        let mut buf = [0; 1];
        loop {
            if let Ok(_) = self.rx.read(&mut buf).await {
                for b in &buf[..] {
                    self.parse_buffer.write(*b).unwrap();
                }
                if let Err(e) = self.digest().await {
                    error!("Error digesting modem input: {:?}", e);
                }
            }
        }
    }

    async fn digest(&mut self) -> Result<(), DriverError> {
        let result = self.parse_buffer.parse();

        if let Ok(response) = result {
            if !matches!(response, AtResponse::None) {
                trace!("--> {:?}", response);
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
                    self.response_producer.send(response).await;
                }
                AtResponse::Closed(..) | AtResponse::DataAvailable { .. } => {
                    self.notification_producer.send(response).await;
                }
                AtResponse::WifiConnected => {
                    debug!("wifi connected");
                }
                AtResponse::WifiDisconnect => {
                    debug!("wifi disconnect");
                }
                AtResponse::GotIp => {
                    debug!("wifi got ip");
                }
            }
        }
        Ok(())
    }
}

impl<'a, TX> Esp8266Controller<'a, TX>
where
    TX: Write,
{
    pub fn new(
        initialized: &'a Initialized,
        tx: TX,
        response_consumer: Receiver<'a, DriverMutex, AtResponse, 2>,
        notification_consumer: Receiver<'a, DriverMutex, AtResponse, 2>,
    ) -> Self {
        Self {
            initialized,
            socket_pool: SocketPool::new(),
            tx,
            response_consumer,
            notification_consumer,
        }
    }

    async fn send<'c>(&mut self, command: Command<'c>) -> Result<AtResponse, DriverError> {
        if self.initialized.wait().await? {
            debug!("Device initialized, setting up parameters");
            self.initialize().await?;
        }
        let mut bytes = command.as_bytes();
        trace!(
            "writing command {}",
            core::str::from_utf8(bytes.as_bytes()).unwrap()
        );

        bytes.push_str("\r\n").unwrap();
        let bs = bytes.as_bytes();

        self.send_recv(&bs).await
    }

    async fn send_recv(&mut self, data: &[u8]) -> Result<AtResponse, DriverError> {
        self.tx
            .write(data)
            .await
            .map_err(|_| DriverError::WriteError)?;
        Ok(self.response_consumer.recv().await)
    }

    async fn initialize(&mut self) -> Result<(), DriverError> {
        // Initialize
        self.send_recv(b"ATE0\r\n")
            .await
            .map_err(|_| DriverError::UnableToInitialize)?;
        self.send_recv(b"AT+CIPMUX=1\r\n")
            .await
            .map_err(|_| DriverError::UnableToInitialize)?;
        self.send_recv(b"AT+CIPRECVMODE=1\r\n")
            .await
            .map_err(|_| DriverError::UnableToInitialize)?;
        self.send_recv(b"AT+CWMODE_CUR=1\r\n")
            .await
            .map_err(|_| DriverError::UnableToInitialize)?;
        Ok(())
    }

    /*
    async fn set_wifi_mode(&self, mode: WiFiMode) -> Result<(), ()> {
        let command = Command::SetMode(mode);
        match self.send(command).await {
            Ok(AtResponse::Ok) => Ok(()),
            _ => Err(()),
        }
    }
    */

    async fn join_wep(&mut self, ssid: &str, password: &str) -> Result<IpAddress, JoinError> {
        let command = Command::JoinAp { ssid, password };
        match self.send(command).await {
            Ok(AtResponse::Ok) => {
                let address = self.get_ip_address().await.map_err(|_| JoinError::Unknown);
                if let Ok(address) = address {
                    info!("Joined network IP address {:?}", address);
                }
                address
            }
            Ok(AtResponse::WifiConnectionFailure(reason)) => {
                warn!("Error connecting to wifi: {:?}", reason);
                Err(JoinError::Unknown)
            }
            Ok(r) => {
                error!("Unexpected response: {:?}", r);
                Err(JoinError::UnableToAssociate)
            }
            Err(e) => {
                error!("Error: {:?}", e);
                Err(JoinError::UnableToAssociate)
            }
        }
    }

    async fn get_ip_address(&mut self) -> Result<IpAddress, ()> {
        let command = Command::QueryIpAddress;

        if let Ok(AtResponse::IpAddresses(addresses)) = self.send(command).await {
            return Ok(IpAddress::V4(addresses.ip));
        }

        Err(())
    }

    fn process_notifications(&mut self) {
        while let Ok(response) = self.notification_consumer.try_recv() {
            match response {
                AtResponse::DataAvailable { .. } => {
                    //  shared.socket_pool // [link_id].available += len;
                }
                AtResponse::Connect(_) => {}
                AtResponse::Closed(link_id) => {
                    self.socket_pool.close(link_id as u8);
                }
                _ => { /* ignore */ }
            }
        }
    }
}

impl<'a, TX> WifiSupplicant for Esp8266Controller<'a, TX>
where
    TX: Write,
{
    type JoinFuture<'m> = impl Future<Output = Result<IpAddress, JoinError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn join<'m>(&'m mut self, join_info: Join<'m>) -> Self::JoinFuture<'m> {
        async move {
            match join_info {
                Join::Open => Err(JoinError::Unknown),
                Join::Wpa { ssid, password } => self.join_wep(ssid, password).await,
            }
        }
    }
}

impl<'a, TX> TcpStack for Esp8266Controller<'a, TX>
where
    TX: Write,
{
    type SocketHandle = u8;

    type OpenFuture<'m> = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            self.socket_pool
                .open()
                .await
                .map_err(|_| TcpError::OpenError)
        }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        _: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            let command = Command::StartConnection(handle as usize, ConnectionType::TCP, dst);
            if let Ok(AtResponse::Connect(..)) = self.send(command).await {
                Ok(())
            } else {
                Err(TcpError::ConnectError)
            }
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.process_notifications();
            if self.socket_pool.is_closed(handle) {
                return Err(TcpError::SocketClosed);
            }
            let command = Command::Send {
                link_id: handle as usize,
                len: buf.len(),
            };

            let result = match self.send(command).await {
                Ok(AtResponse::Ok) => {
                    match self.response_consumer.recv().await {
                        AtResponse::ReadyForData => {
                            self.tx.write(buf).await.map_err(|_| TcpError::WriteError)?;
                            let mut data_sent: Option<usize> = None;
                            loop {
                                match self.response_consumer.recv().await {
                                    AtResponse::ReceivedDataToSend(len) => {
                                        data_sent.replace(len);
                                    }
                                    AtResponse::SendOk => break Ok(data_sent.unwrap_or_default()),
                                    _ => {
                                        break Err(TcpError::WriteError);
                                        // unknown response
                                    }
                                }
                            }
                        }
                        r => {
                            warn!("Unexpected response: {:?}", r);
                            Err(TcpError::WriteError)
                        }
                    }
                }
                Ok(r) => {
                    warn!("Unexpected response: {:?}", r);
                    Err(TcpError::WriteError)
                }
                Err(_) => Err(TcpError::WriteError),
            };
            result
        }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            const BLOCK_SIZE: usize = 512;
            let mut rp = 0;
            let mut remaining = buf.len();
            while remaining > 0 {
                let result = async {
                    self.process_notifications();
                    if self.socket_pool.is_closed(handle) {
                        return Err(TcpError::SocketClosed);
                    }

                    let recv_len = core::cmp::min(remaining, BLOCK_SIZE);
                    let command = Command::Receive {
                        link_id: handle as usize,
                        len: recv_len,
                    };
                    //                   info!("Awaiting {} bytes from adapter", recv_len);

                    match self.send(command).await {
                        Ok(AtResponse::DataReceived(inbound, len)) => {
                            for (i, b) in inbound[0..len].iter().enumerate() {
                                buf[rp + i] = *b;
                            }
                            //                            info!("Received {} bytes from adapter", len);
                            Ok(len)
                        }
                        Ok(AtResponse::Ok) => Ok(0),
                        Ok(r) => {
                            warn!("Unexpected response: {:?}", r);
                            Err(TcpError::ReadError)
                        }
                        Err(e) => {
                            warn!("Unexpected error: {:?}", e);
                            Err(TcpError::ReadError)
                        }
                    }
                }
                .await;

                match result {
                    Ok(len) => {
                        rp += len;
                        remaining -= len;
                        if len == 0 {
                            return Ok(rp);
                        }
                    }
                    Err(e) => {
                        if rp == 0 {
                            return Err(e);
                        } else {
                            return Ok(rp);
                        }
                    }
                }
            }
            Ok(rp)
        }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.socket_pool.close(handle);
            let command = Command::CloseConnection(handle as usize);
            match self.send(command).await {
                Ok(AtResponse::Ok) | Ok(AtResponse::UnlinkFail) => {
                    self.socket_pool.close(handle);
                    Ok(())
                }
                _ => Err(TcpError::CloseError),
            }
        }
    }
}
