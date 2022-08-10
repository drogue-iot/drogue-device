//! Esp8266 Async Driver
//!
//! An async driver for the Esp8266 AT-command firmware. The driver implements the drogue-network APIs for
//! WifiSupplicant and TcpStack.

mod buffer;
mod num;
mod parser;
mod protocol;

use crate::traits::wifi::{Join, JoinError, WifiSupplicant};
use atomic_polyfill::{AtomicBool, Ordering};
use buffer::Buffer;
use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use embassy_executor::time::{Duration, Timer};
use embassy_util::{
    blocking_mutex::raw::NoopRawMutex,
    channel::mpmc::{Channel, DynamicReceiver, DynamicSender},
};
use embassy_util::{select3, Either3};
use embedded_hal::digital::v2::OutputPin;
use embedded_io::asynch::{Read, Write};
use embedded_nal_async::*;
use futures_intrusive::sync::LocalMutex;
use heapless::spsc::Queue;
use protocol::{Command, ConnectionType, Response as AtResponse};

pub const BUFFER_LEN: usize = 512;
type DriverMutex = NoopRawMutex;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DriverError {
    NoSocket,
    UnableToInitialize,
    NoAvailableSockets,
    Timeout,
    OpenError,
    ConnectError,
    WriteError,
    ReadError,
    CloseError,
    SocketClosed,
    InvalidSocket,
    OperationNotSupported,
    JoinError(JoinError),
}

struct Inner<T> {
    transport: T,
    parse_buffer: Buffer,
    inbound: Queue<AtResponse, 4>,
}

impl<T> Inner<T>
where
    T: Read + Write,
{
    fn digest(
        &mut self,
        notifications: &dyn SocketsNotifier,
    ) -> Result<Option<AtResponse>, DriverError> {
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
                AtResponse::Closed(link_id) => {
                    notifications.notify(link_id, response);
                }
                AtResponse::DataAvailable { link_id, len: _ } => {
                    notifications.notify(link_id, response);
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

    async fn send_command<'c>(
        &mut self,
        command: Command<'c>,
        notifications: &dyn SocketsNotifier,
    ) -> Result<AtResponse, DriverError> {
        let mut bytes = command.as_bytes();
        trace!(
            "writing command {}",
            core::str::from_utf8(bytes.as_bytes()).unwrap()
        );

        bytes.push_str("\r\n").unwrap();
        let bs = bytes.as_bytes();

        self.send_recv(&bs, notifications).await
    }

    async fn receive_response(
        &mut self,
        notifications: &dyn SocketsNotifier,
    ) -> Result<AtResponse, DriverError> {
        loop {
            if let Some(r) = self.inbound.dequeue() {
                return Ok(r);
            }
            let mut buf = [0; 1];
            if let Ok(len) = self.transport.read(&mut buf).await {
                for b in &buf[..len] {
                    self.parse_buffer.write(*b).unwrap();
                }
                if let Some(response) = self.digest(notifications)? {
                    let _ = self.inbound.enqueue(response);
                }
            }
        }
    }

    async fn process(&mut self, notifications: &dyn SocketsNotifier) -> Result<(), DriverError> {
        if self.inbound.is_empty() {
            let mut buf = [0; 1];
            if let Ok(len) = self.transport.read(&mut buf).await {
                for b in &buf[..len] {
                    self.parse_buffer.write(*b).unwrap();
                }
                if let Some(response) = self.digest(notifications)? {
                    let _ = self.inbound.enqueue(response);
                }
            }
        }
        Ok(())
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), DriverError> {
        self.transport
            .write(data)
            .await
            .map_err(|_| DriverError::WriteError)?;
        Ok(())
    }

    async fn send_recv(
        &mut self,
        data: &[u8],
        notifications: &dyn SocketsNotifier,
    ) -> Result<AtResponse, DriverError> {
        self.transport
            .write(data)
            .await
            .map_err(|_| DriverError::WriteError)?;
        self.receive_response(notifications).await
    }
}

pub struct Esp8266Handle<T>
where
    T: Read + Write,
{
    inner: LocalMutex<Inner<T>>,
}

impl<T> Esp8266Handle<T>
where
    T: Read + Write,
{
    async fn configure(&self, notifications: &dyn SocketsNotifier) -> Result<(), DriverError> {
        // Initialize
        let mut inner = self.inner.lock().await;
        let to_init_error = |_| DriverError::UnableToInitialize;
        inner
            .send_recv(b"ATE0\r\n", notifications)
            .await
            .map_err(to_init_error)?;
        inner
            .send_recv(b"AT+CIPMUX=1\r\n", notifications)
            .await
            .map_err(to_init_error)?;
        inner
            .send_recv(b"AT+CIPRECVMODE=1\r\n", notifications)
            .await
            .map_err(to_init_error)?;
        inner
            .send_recv(b"AT+CWMODE_CUR=1\r\n", notifications)
            .await
            .map_err(to_init_error)?;
        Ok(())
    }

    async fn join_wep(
        &self,
        ssid: &str,
        password: &str,
        notifications: &dyn SocketsNotifier,
    ) -> Result<IpAddr, JoinError> {
        let mut inner = self.inner.lock().await;
        let command = Command::JoinAp { ssid, password };
        match inner.send_command(command, notifications).await {
            Ok(AtResponse::Ok) => {
                let command = Command::QueryIpAddress;
                if let Ok(AtResponse::IpAddresses(addresses)) =
                    inner.send_command(command, notifications).await
                {
                    Ok(IpAddr::V4(addresses.ip))
                } else {
                    Err(JoinError::Unknown)
                }
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

    async fn send(
        &self,
        id: usize,
        buf: &[u8],
        notifications: &dyn SocketsNotifier,
    ) -> Result<usize, DriverError> {
        let command = Command::Send {
            link_id: id,
            len: buf.len(),
        };
        let mut inner = self.inner.lock().await;
        debug!("[{}] in send", id);
        let result = match inner.send_command(command, notifications).await {
            Ok(AtResponse::Ok) => {
                match inner.receive_response(notifications).await? {
                    AtResponse::ReadyForData => {
                        inner.write_data(buf).await?;
                        let mut data_sent: Option<usize> = None;
                        loop {
                            match inner.receive_response(notifications).await? {
                                AtResponse::ReceivedDataToSend(len) => {
                                    data_sent.replace(len);
                                }
                                AtResponse::SendOk => break Ok(data_sent.unwrap_or_default()),
                                _ => {
                                    break Err(DriverError::WriteError);
                                    // unknown response
                                }
                            }
                        }
                    }
                    r => {
                        warn!("Unexpected response: {:?}", r);
                        Err(DriverError::WriteError)
                    }
                }
            }
            Ok(r) => {
                warn!("Unexpected response: {:?}", r);
                Err(DriverError::WriteError)
            }
            Err(_) => Err(DriverError::WriteError),
        };
        result
    }

    async fn receive(
        &self,
        id: usize,
        buf: &mut [u8],
        notifications: &dyn SocketsNotifier,
    ) -> Result<usize, DriverError> {
        let mut inner = self.inner.lock().await;
        debug!("[{}] in receive", id);
        const BLOCK_SIZE: usize = 512;
        let mut rp = 0;
        let mut remaining = buf.len();
        while remaining > 0 {
            let result = async {
                let recv_len = core::cmp::min(remaining, BLOCK_SIZE);
                let command = Command::Receive {
                    link_id: id,
                    len: recv_len,
                };
                match inner.send_command(command, notifications).await {
                    Ok(AtResponse::DataReceived(inbound, len)) => {
                        for (i, b) in inbound[0..len].iter().enumerate() {
                            buf[rp + i] = *b;
                        }
                        trace!("Received {} bytes from adapter", len);
                        Ok(len)
                    }
                    Ok(AtResponse::Ok) => Ok(0),
                    Ok(r) => {
                        warn!("Unexpected response: {:?}", r);
                        Err(DriverError::ReadError)
                    }
                    Err(e) => {
                        warn!("Unexpected error: {:?}", e);
                        Err(DriverError::ReadError)
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

    async fn process(&self, notifications: &dyn SocketsNotifier) -> Result<(), DriverError> {
        let mut inner = self.inner.lock().await;
        inner.process(notifications).await?;
        Ok(())
    }

    async fn connect_client(
        &self,
        id: usize,
        remote: SocketAddr,
        notifications: &dyn SocketsNotifier,
    ) -> Result<(), DriverError> {
        let mut inner = self.inner.lock().await;
        debug!("[{}] in connect_client", id);
        let command = Command::StartConnection(id as usize, ConnectionType::TCP, remote);
        if let Ok(AtResponse::Connect(..)) = inner.send_command(command, notifications).await {
            debug!("[{}] connected!", id);
            Ok(())
        } else {
            Err(DriverError::ConnectError)
        }
    }

    async fn close_socket(
        &self,
        id: usize,
        notifications: &dyn SocketsNotifier,
    ) -> Result<(), DriverError> {
        debug!("[{}] in drop/close", id);
        let mut inner = self.inner.lock().await;
        let command = Command::CloseConnection(id);
        match inner.send_command(command, notifications).await {
            Ok(AtResponse::Ok) | Ok(AtResponse::UnlinkFail) => Ok(()),
            _ => Err(DriverError::CloseError),
        }
    }
}

pub struct Esp8266Modem<'a, T, ENABLE, RESET, const MAX_SOCKETS: usize>
where
    T: Read + Write,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    sockets: [AtomicBool; MAX_SOCKETS],
    handle: Esp8266Handle<T>,
    enable: RefCell<ENABLE>,
    reset: RefCell<RESET>,
    notifications: [Channel<DriverMutex, AtResponse, 2>; MAX_SOCKETS],
    control: Channel<DriverMutex, Control, 2>,
    _a: PhantomData<&'a T>,
}

impl<'a, T, ENABLE, RESET, const MAX_SOCKETS: usize> Esp8266Modem<'a, T, ENABLE, RESET, MAX_SOCKETS>
where
    T: Read + Write,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    pub fn new(transport: T, enable: ENABLE, reset: RESET) -> Self {
        const C: Channel<DriverMutex, AtResponse, 2> = Channel::new();
        const UNUSED: AtomicBool = AtomicBool::new(false);
        Self {
            handle: Esp8266Handle {
                inner: LocalMutex::new(
                    Inner {
                        transport,
                        parse_buffer: Buffer::new(),
                        inbound: Queue::new(),
                    },
                    true,
                ),
            },
            sockets: [UNUSED; MAX_SOCKETS],
            enable: RefCell::new(enable),
            reset: RefCell::new(reset),
            control: Channel::new(),
            notifications: [C; MAX_SOCKETS],
            _a: PhantomData,
        }
    }

    async fn initialize(&self) -> Result<(), DriverError> {
        self.enable.borrow_mut().set_low().ok().unwrap();
        self.reset.borrow_mut().set_low().ok().unwrap();
        let mut buffer: [u8; 1024] = [0; 1024];
        let mut pos = 0;

        const READY: [u8; 7] = *b"ready\r\n";

        info!("Initializing ESP8266");
        self.enable.borrow_mut().set_high().ok().unwrap();
        self.reset.borrow_mut().set_high().ok().unwrap();

        let mut rx_buf = [0; 1];
        loop {
            let result = {
                self.handle
                    .inner
                    .lock()
                    .await
                    .transport
                    .read(&mut rx_buf[..])
                    .await
            };
            match result {
                Ok(_) => {
                    buffer[pos] = rx_buf[0];
                    pos += 1;
                    if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                        self.handle.configure(&self.notifications).await?;
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

    pub fn new_socket(&'a self) -> Result<Esp8266Socket<'a, T>, DriverError> {
        for id in 0..MAX_SOCKETS {
            if self.sockets[id].swap(true, Ordering::SeqCst) == false {
                debug!("[{}] client created", id);
                let notifications = self.notifications[id].receiver().into();
                return Ok(Esp8266Socket {
                    id,
                    handle: &self.handle,
                    notifier: &self.notifications,
                    notifications,
                    control: self.control.sender().into(),
                    state: SocketState::Open,
                    available: 0,
                    buffer: Buf::new(),
                });
            }
        }
        Err(DriverError::NoSocket)
    }

    pub async fn run(&'a self, ssid: &'a str, psk: &'a str) -> Result<(), DriverError> {
        self.initialize().await?;
        self.handle
            .join_wep(ssid, psk, &self.notifications)
            .await
            .map_err(DriverError::JoinError)?;
        loop {
            let t = Timer::after(Duration::from_secs(1));
            match select3(
                self.control.recv(),
                t,
                self.handle.process(&self.notifications),
            )
            .await
            {
                Either3::First(control) => match control {
                    Control::Close(id) => {
                        let _ = self.handle.close_socket(id, &self.notifications).await;
                        self.sockets[id].store(false, Ordering::SeqCst);
                    }
                },
                Either3::Second(_) => {}
                Either3::Third(result) => match result {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Error processing events: {:?}", e);
                    }
                },
            }
        }
    }
}

enum Control {
    Close(usize),
}

pub trait SocketsNotifier {
    fn notify(&self, link_id: usize, response: AtResponse);
}

impl<const MAX_SOCKETS: usize> SocketsNotifier
    for [Channel<DriverMutex, AtResponse, 2>; MAX_SOCKETS]
{
    fn notify(&self, link_id: usize, response: AtResponse) {
        debug!("[{}] Got notification: {:?}", link_id, response);
        if let Some(s) = &self.get(link_id) {
            let r = s.try_send(response);
            debug!("[{}] notification to link id result: {:?}", link_id, r);
        }
    }
}

pub struct Esp8266Socket<'a, T>
where
    T: Read + Write,
{
    id: usize,
    handle: &'a Esp8266Handle<T>,
    notifier: &'a dyn SocketsNotifier,
    notifications: DynamicReceiver<'a, AtResponse>,
    control: DynamicSender<'a, Control>,
    state: SocketState,
    available: usize,
    buffer: Buf<BUFSIZE>,
}

const BUFSIZE: usize = 1500;

#[derive(PartialEq, Clone, Copy)]
enum SocketState {
    Closed,
    Open,
    Connected,
}

impl Default for SocketState {
    fn default() -> Self {
        Self::Open
    }
}

impl<'a, T> Esp8266Socket<'a, T> where T: Read + Write {}

impl<'a, T> embedded_io::Io for Esp8266Socket<'a, T>
where
    T: Read + Write,
{
    type Error = DriverError;
}

impl embedded_io::Error for DriverError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl<'a, T, ENABLE, RESET, const MAX_SOCKETS: usize> embedded_nal_async::TcpConnect
    for Esp8266Modem<'a, T, ENABLE, RESET, MAX_SOCKETS>
where
    T: Read + Write,
    ENABLE: OutputPin,
    RESET: OutputPin,
{
    type Error = DriverError;
    type Connection<'m> = Esp8266Socket<'m, T> where Self: 'm;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection<'m>, Self::Error>> + 'm
	where
		Self: 'm;
    fn connect<'m>(&'m self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            let mut socket = self.new_socket()?;
            socket.process_notifications();
            socket
                .handle
                .connect_client(socket.id, remote, socket.notifier)
                .await?;
            socket.state = SocketState::Connected;
            Ok(socket)
        }
    }
}

impl<'a, T> Esp8266Socket<'a, T>
where
    T: Read + Write,
{
    fn close(&mut self) {
        match self.state {
            SocketState::Closed => {
                self.state = SocketState::Open;
            }
            SocketState::Open | SocketState::Connected => {
                self.state = SocketState::Closed;
            }
        }
    }

    fn is_closed(&self) -> bool {
        self.state == SocketState::Closed
    }

    fn process_notifications(&mut self) {
        while let Ok(response) = self.notifications.try_recv() {
            self.process_notification(response);
        }
    }

    fn process_notification(&mut self, response: AtResponse) {
        match response {
            AtResponse::DataAvailable { link_id: _, len } => {
                self.available += len;
            }
            AtResponse::Closed(_) => {
                self.close();
            }
            _ => { /* ignore */ }
        }
    }

    async fn wait_available(&mut self) -> Result<(), DriverError> {
        debug!(
            "[{}] waiting for data (available = {})",
            self.id, self.available
        );
        while self.available == 0 && !self.is_closed() {
            let response = self.notifications.recv().await;
            self.process_notification(response);
            self.process_notifications();
        }
        Ok(())
    }
}

struct Buf<const SZ: usize> {
    buf: [u8; SZ],
    wp: usize,
}

impl<const SZ: usize> Buf<SZ> {
    fn new() -> Self {
        Self {
            buf: [0; SZ],
            wp: 0,
        }
    }

    fn reduce(&mut self, nbytes: usize) {
        for i in 0..nbytes {
            self.buf[i] = self.buf[i + nbytes - 1];
        }
        self.wp -= nbytes;
    }

    fn write(&mut self, buf: &[u8]) -> usize {
        let to_copy = core::cmp::min(buf.len(), self.buf.len() - self.wp);
        self.buf[self.wp..self.wp + to_copy].copy_from_slice(&buf[..to_copy]);
        self.wp += to_copy;
        to_copy
    }

    fn slice(&self) -> &[u8] {
        &self.buf[..self.wp]
    }
}

impl<'a, T> embedded_io::asynch::Write for Esp8266Socket<'a, T>
where
    T: Read + Write + 'a,
{
    type WriteFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    /// Write a buffer into this writer, returning how many bytes were written.
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.process_notifications();
            if self.is_closed() {
                return Err(DriverError::SocketClosed);
            }

            let mut written = self.buffer.write(buf);
            while written < buf.len() {
                self.flush().await?;
                written += self.buffer.write(&buf[written..]);
            }
            Ok(written)
        }
    }

    /// Future returned by `flush`.
    type FlushFuture<'m> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    /// Flush this output stream, ensuring that all intermediately buffered contents reach their destination.
    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move {
            let written = self.buffer.slice();
            let written = self.handle.send(self.id, written, self.notifier).await?;
            self.buffer.reduce(written);
            Ok(())
        }
    }
}

impl<'a, T> embedded_io::asynch::Read for Esp8266Socket<'a, T>
where
    T: Read + Write + 'a,
{
    type ReadFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    /// Pull some bytes from this source into the specified buffer, returning how many bytes were read.
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            self.wait_available().await?;
            self.process_notifications();
            if self.is_closed() {
                return Err(DriverError::SocketClosed);
            }
            // Read available data
            let to_read = core::cmp::min(buf.len(), self.available);
            debug!("[{}] receiving {} bytes", self.id, to_read);
            let r = self
                .handle
                .receive(self.id, &mut buf[..to_read], self.notifier)
                .await?;
            self.available -= r;
            Ok(r)
        }
    }
}

impl<'a, T> Drop for Esp8266Socket<'a, T>
where
    T: Read + Write + 'a,
{
    fn drop(&mut self) {
        self.close();
        if let Ok(_) = self.control.try_send(Control::Close(self.id)) {
            self.close();
        }
    }
}

impl<'a, T, ENABLE, RESET, const MAX_SOCKETS: usize> WifiSupplicant
    for Esp8266Modem<'a, T, ENABLE, RESET, MAX_SOCKETS>
where
    T: Read + Write + 'a,
    ENABLE: OutputPin + 'a,
    RESET: OutputPin + 'a,
{
    type JoinFuture<'m> = impl Future<Output = Result<IpAddr, JoinError>> + 'm
    where
        Self: 'm;
    fn join<'m>(&'m mut self, join_info: Join<'m>) -> Self::JoinFuture<'m> {
        async move {
            match join_info {
                Join::Open => Err(JoinError::Unknown),
                Join::Wpa { ssid, password } => {
                    self.handle
                        .join_wep(ssid, password, &self.notifications)
                        .await
                }
            }
        }
    }
}
