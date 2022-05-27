//! Esp8266 Async Driver
//!
//! An async driver for the Esp8266 AT-command firmware. The driver implements the drogue-network APIs for
//! WifiSupplicant and TcpStack.

mod buffer;
mod num;
mod parser;
mod protocol;

use crate::drivers::common::socket_pool::SocketPool;

use crate::network::tcp::TcpError;
use crate::traits::wifi::{Join, JoinError, WifiSupplicant};
use buffer::Buffer;
use core::future::Future;
use embassy::{
    blocking_mutex::raw::NoopRawMutex,
    channel::Channel,
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt},
};
use embedded_hal::digital::v2::OutputPin;
use embedded_nal_async::*;
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

pub struct Esp8266Modem<T, ENABLE, RESET>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    transport: T,
    enable: ENABLE,
    reset: RESET,
    parse_buffer: Buffer,
    socket_pool: SocketPool,
    notifications: Channel<DriverMutex, AtResponse, 2>,
}

impl<T, ENABLE, RESET> Esp8266Modem<T, ENABLE, RESET>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    pub fn new(transport: T, enable: ENABLE, reset: RESET) -> Self {
        Self {
            transport,
            enable,
            reset,
            notifications: Channel::new(),
            socket_pool: SocketPool::new(),
            parse_buffer: Buffer::new(),
        }
    }

    pub async fn initialize(&mut self) -> Result<(), DriverError> {
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
            let result = self.transport.read(&mut rx_buf[..]).await;
            match result {
                Ok(_) => {
                    buffer[pos] = rx_buf[0];
                    pos += 1;
                    if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                        info!("ESP8266 initialized");
                        self.configure().await?;
                        info!("ESP8266 configured");
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

    fn digest(&mut self) -> Result<Option<AtResponse>, DriverError> {
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
                | AtResponse::IpAddresses(..) => return Ok(Some(response)),
                AtResponse::Closed(..) | AtResponse::DataAvailable { .. } => {
                    let _ = self.notifications.try_send(response);
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
        Ok(None)
    }

    async fn send<'c>(&mut self, command: Command<'c>) -> Result<AtResponse, DriverError> {
        let mut bytes = command.as_bytes();
        trace!(
            "writing command {}",
            core::str::from_utf8(bytes.as_bytes()).unwrap()
        );

        bytes.push_str("\r\n").unwrap();
        let bs = bytes.as_bytes();

        self.send_recv(&bs).await
    }

    async fn receive(&mut self) -> Result<AtResponse, DriverError> {
        let mut buf = [0; 1];
        loop {
            if let Ok(_) = self.transport.read(&mut buf).await {
                for b in &buf[..] {
                    self.parse_buffer.write(*b).unwrap();
                }
                if let Some(response) = self.digest()? {
                    return Ok(response);
                }
            }
        }
    }

    async fn send_recv(&mut self, data: &[u8]) -> Result<AtResponse, DriverError> {
        self.transport
            .write(data)
            .await
            .map_err(|_| DriverError::WriteError)?;
        self.receive().await
    }

    async fn configure(&mut self) -> Result<(), DriverError> {
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

    async fn join_wep(&mut self, ssid: &str, password: &str) -> Result<IpAddr, JoinError> {
        let command = Command::JoinAp { ssid, password };
        match self.send(command).await {
            Ok(AtResponse::Ok) => self.get_ip_address().await.map_err(|_| JoinError::Unknown),
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

    async fn get_ip_address(&mut self) -> Result<IpAddr, ()> {
        let command = Command::QueryIpAddress;

        if let Ok(AtResponse::IpAddresses(addresses)) = self.send(command).await {
            return Ok(IpAddr::V4(addresses.ip));
        }

        Err(())
    }

    fn process_notifications(&mut self) {
        while let Ok(response) = self.notifications.try_recv() {
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

impl<T, ENABLE, RESET> WifiSupplicant for Esp8266Modem<T, ENABLE, RESET>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    type JoinFuture<'m> = impl Future<Output = Result<IpAddr, JoinError>> + 'm
    where
        Self: 'm;
    fn join<'m>(&'m mut self, join_info: Join<'m>) -> Self::JoinFuture<'m> {
        async move {
            match join_info {
                Join::Open => Err(JoinError::Unknown),
                Join::Wpa { ssid, password } => self.join_wep(ssid, password).await,
            }
        }
    }
}

impl<T, ENABLE, RESET> TcpClientStack for Esp8266Modem<T, ENABLE, RESET>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    type TcpSocket = u8;
    type Error = TcpError;

    type SocketFuture<'m> = impl Future<Output = Result<Self::TcpSocket, Self::Error>> + 'm
    where
        Self: 'm;
    fn socket<'m>(&'m mut self) -> Self::SocketFuture<'m> {
        async move {
            self.socket_pool
                .open()
                .await
                .map_err(|_| TcpError::OpenError)
        }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: &'m mut Self::TcpSocket,
        remote: SocketAddr,
    ) -> Self::ConnectFuture<'m> {
        async move {
            let command = Command::StartConnection(*handle as usize, ConnectionType::TCP, remote);
            if let Ok(AtResponse::Connect(..)) = self.send(command).await {
                Ok(())
            } else {
                Err(TcpError::ConnectError)
            }
        }
    }

    type IsConnectedFuture<'m> =
        impl Future<Output = Result<bool, Self::Error>> + 'm where Self: 'm;
    fn is_connected<'m>(&'m mut self, handle: &'m Self::TcpSocket) -> Self::IsConnectedFuture<'m> {
        async move { Ok(!self.socket_pool.is_closed(*handle)) }
    }

    type SendFuture<'m> =
        impl Future<Output = Result<usize, Self::Error>> + 'm where Self: 'm;
    fn send<'m>(
        &'m mut self,
        handle: &'m mut Self::TcpSocket,
        buf: &'m [u8],
    ) -> Self::SendFuture<'m> {
        async move {
            self.process_notifications();
            if self.socket_pool.is_closed(*handle) {
                return Err(TcpError::SocketClosed);
            }
            let command = Command::Send {
                link_id: *handle as usize,
                len: buf.len(),
            };

            let result = match self.send(command).await {
                Ok(AtResponse::Ok) => {
                    match self.receive().await.map_err(|_| TcpError::WriteError)? {
                        AtResponse::ReadyForData => {
                            self.transport
                                .write(buf)
                                .await
                                .map_err(|_| TcpError::WriteError)?;
                            let mut data_sent: Option<usize> = None;
                            loop {
                                match self.receive().await.map_err(|_| TcpError::WriteError)? {
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

    type ReceiveFuture<'m> =
        impl Future<Output = Result<usize, Self::Error>> + 'm where Self: 'm;
    fn receive<'m>(
        &'m mut self,
        handle: &'m mut Self::TcpSocket,
        buf: &'m mut [u8],
    ) -> Self::ReceiveFuture<'m> {
        async move {
            const BLOCK_SIZE: usize = 512;
            let mut rp = 0;
            let mut remaining = buf.len();
            while remaining > 0 {
                let result = async {
                    self.process_notifications();
                    if self.socket_pool.is_closed(*handle) {
                        return Err(TcpError::SocketClosed);
                    }

                    let recv_len = core::cmp::min(remaining, BLOCK_SIZE);
                    let command = Command::Receive {
                        link_id: *handle as usize,
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

    type CloseFuture<'m> =
        impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn close<'m>(&'m mut self, handle: Self::TcpSocket) -> Self::CloseFuture<'m> {
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
