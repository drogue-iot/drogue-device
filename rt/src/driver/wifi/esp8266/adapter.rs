use embedded_hal::{digital::v2::OutputPin, serial::Read, serial::Write};

use crate::protocol::{
    Command, ConnectionType, FirmwareInfo, IpAddresses, ResolverAddresses, Response, WiFiMode,
    WifiConnectionFailure,
};

use heapless::{
    consts::{U16, U2},
    spsc::{Consumer, Queue},
    String,
};

use log::info;

use crate::adapter::AdapterError::UnableToInitialize;
use crate::ingress::Ingress;
use crate::network::Esp8266IpNetworkDriver;
use crate::protocol::Response::IpAddress;
use core::fmt::Debug;
use drogue_network::addr::{HostAddr, HostSocketAddr, Ipv4Addr};
use drogue_network::dns::DnsError;
use nom::lib::std::fmt::Formatter;

#[derive(Debug)]
enum SocketState {
    HalfClosed,
    Closed,
    Open,
    Connected,
}

type Initialized<'a, Tx, Rx> = (Adapter<'a, Tx>, Ingress<'a, Rx>);

/// Initialize an ESP8266 board for usage as a Wifi-offload device.
///
/// * tx: Serial transmitter.
/// * rx: Serial receiver.
/// * enable_pin: Pin connected to the ESP's `en` pin.
/// * reset_pin: Pin connect to the ESP's `rst` pin.
/// * response_queue: Queue for inbound AT command responses.
/// * notification_queue: Queue for inbound unsolicited AT notification messages.
pub fn initialize<'a, Tx, Rx, EnablePin, ResetPin>(
    mut tx: Tx,
    mut rx: Rx,
    enable_pin: &mut EnablePin,
    reset_pin: &mut ResetPin,
    response_queue: &'a mut Queue<Response, U2>,
    notification_queue: &'a mut Queue<Response, U16>,
) -> Result<Initialized<'a, Tx, Rx>, AdapterError>
where
    Tx: Write<u8>,
    Rx: Read<u8>,
    EnablePin: OutputPin,
    ResetPin: OutputPin,
{
    let mut buffer: [u8; 1024] = [0; 1024];
    let mut pos = 0;

    const READY: [u8; 7] = *b"ready\r\n";

    let mut counter = 0;

    enable_pin
        .set_high()
        .map_err(|_| AdapterError::UnableToInitialize)?;
    reset_pin
        .set_high()
        .map_err(|_| AdapterError::UnableToInitialize)?;

    log::debug!("waiting for adapter to become ready");

    loop {
        let result = rx.read();
        match result {
            Ok(c) => {
                buffer[pos] = c;
                pos += 1;
                if pos >= READY.len() && buffer[pos - READY.len()..pos] == READY {
                    log::debug!("adapter is ready");
                    disable_echo(&mut tx, &mut rx)?;
                    enable_mux(&mut tx, &mut rx)?;
                    set_recv_mode(&mut tx, &mut rx)?;
                    return Ok(build_adapter_and_ingress(
                        tx,
                        rx,
                        response_queue,
                        notification_queue,
                    ));
                }
            }
            Err(nb::Error::WouldBlock) => {
                continue;
            }
            Err(_) if counter > 10_000 => {
                break;
            }
            Err(_) => {
                counter += 1;
            }
        }
    }

    Err(AdapterError::UnableToInitialize)
}

fn build_adapter_and_ingress<'a, Tx, Rx>(
    tx: Tx,
    rx: Rx,
    response_queue: &'a mut Queue<Response, U2>,
    notification_queue: &'a mut Queue<Response, U16>,
) -> Initialized<'a, Tx, Rx>
where
    Tx: Write<u8>,
    Rx: Read<u8>,
{
    let (response_producer, response_consumer) = response_queue.split();
    let (notification_producer, notification_consumer) = notification_queue.split();
    (
        Adapter {
            tx,
            response_consumer,
            notification_consumer,
            sockets: initialize_sockets(),
        },
        Ingress::new(rx, response_producer, notification_producer),
    )
}

fn initialize_sockets() -> [Socket; 5] {
    [
        Socket::new(),
        Socket::new(),
        Socket::new(),
        Socket::new(),
        Socket::new(),
    ]
}

struct Socket {
    state: SocketState,
    available: usize,
}

impl Socket {
    fn new() -> Self {
        Self {
            state: SocketState::Closed,
            available: 0,
        }
    }

    #[allow(dead_code)]
    pub fn is_closed(&self) -> bool {
        matches!(self.state, SocketState::Closed)
    }

    #[allow(dead_code)]
    pub fn is_half_closed(&self) -> bool {
        matches!(self.state, SocketState::HalfClosed)
    }

    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        matches!(self.state, SocketState::Open)
    }

    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        matches!(self.state, SocketState::Connected)
    }
}

pub struct Adapter<'a, Tx>
where
    Tx: Write<u8>,
{
    tx: Tx,
    response_consumer: Consumer<'a, Response, U2>,
    notification_consumer: Consumer<'a, Response, U16>,
    sockets: [Socket; 5],
}

impl<'a, Tx> Debug for Adapter<'a, Tx>
where
    Tx: Write<u8>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Adapter").finish()
    }
}

impl<'a, Tx> Adapter<'a, Tx>
where
    Tx: Write<u8>,
{
    fn send<'c>(&mut self, command: Command<'c>) -> Result<Response, AdapterError> {
        let bytes = command.as_bytes();

        info!(
            "writing command {}",
            core::str::from_utf8(bytes.as_bytes()).unwrap()
        );
        for b in bytes.as_bytes().iter() {
            nb::block!(self.tx.write(*b)).map_err(|_| AdapterError::WriteError)?;
        }
        nb::block!(self.tx.write(b'\r')).map_err(|_| AdapterError::WriteError)?;
        nb::block!(self.tx.write(b'\n')).map_err(|_| AdapterError::WriteError)?;
        self.wait_for_response()
    }

    fn wait_for_response(&mut self) -> Result<Response, AdapterError> {
        loop {
            // busy loop until a response is received.
            if let Some(response) = self.response_consumer.dequeue() {
                return Ok(response);
            }
        }
    }

    /// Retrieve the firmware version for the adapter.
    pub fn get_firmware_info(&mut self) -> Result<FirmwareInfo, ()> {
        let command = Command::QueryFirmwareInfo;

        if let Ok(Response::FirmwareInfo(info)) = self.send(command) {
            return Ok(info);
        }

        Err(())
    }

    /// Get the board's IP address. Only valid if connected to an access-point.
    pub fn get_ip_address(&mut self) -> Result<IpAddresses, ()> {
        let command = Command::QueryIpAddress;

        if let Ok(Response::IpAddresses(addresses)) = self.send(command) {
            return Ok(addresses);
        }

        Err(())
    }

    /// Set the mode of the Wi-Fi stack
    ///
    /// Must be done before joining an access point.
    pub fn set_mode(&mut self, mode: WiFiMode) -> Result<(), ()> {
        let command = Command::SetMode(mode);

        match self.send(command) {
            Ok(Response::Ok) => Ok(()),
            _ => Err(()),
        }
    }

    /// Join a wifi access-point.
    ///
    /// The board will expect to obtain an IP address from DHCP.
    ///
    /// * `ssid`: The access-point's SSID to join
    /// * `password`: The password for the access-point.
    pub fn join<'c>(
        &mut self,
        ssid: &'c str,
        password: &'c str,
    ) -> Result<(), WifiConnectionFailure> {
        let command = Command::JoinAp { ssid, password };

        match self.send(command) {
            Ok(Response::Ok) => Ok(()),
            Ok(Response::WifiConnectionFailure(reason)) => Err(reason),
            _ => Err(WifiConnectionFailure::ConnectionFailed),
        }
    }

    pub fn query_dns_resolvers(&mut self) -> Result<ResolverAddresses, ()> {
        let command = Command::QueryDnsResolvers;
        if let Ok(Response::Resolvers(resolvers)) = self.send(command) {
            Ok(resolvers)
        } else {
            Err(())
        }
    }

    pub fn set_dns_resolvers(
        &mut self,
        resolver1: Ipv4Addr,
        resolver2: Option<Ipv4Addr>,
    ) -> Result<(), ()> {
        let command = Command::SetDnsResolvers(ResolverAddresses {
            resolver1,
            resolver2,
        });

        if let Ok(Response::Ok) = self.send(command) {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Consume the adapter and produce a `NetworkStack`.
    pub fn into_network_stack(self) -> Esp8266IpNetworkDriver<'a, Tx> {
        Esp8266IpNetworkDriver::new(self)
    }

    // ----------------------------------------------------------------------
    // TCP Stack
    // ----------------------------------------------------------------------

    fn process_notifications(&mut self) {
        while let Some(response) = self.notification_consumer.dequeue() {
            match response {
                Response::DataAvailable { link_id, len } => {
                    self.sockets[link_id].available += len;
                }
                Response::Connect(_) => {}
                Response::Closed(link_id) => {
                    match self.sockets[link_id].state {
                        SocketState::HalfClosed => {
                            self.sockets[link_id].state = SocketState::Closed;
                        }
                        SocketState::Open | SocketState::Connected => {
                            self.sockets[link_id].state = SocketState::HalfClosed;
                        }
                        SocketState::Closed => {
                            // nothing
                        }
                    }
                }
                _ => { /* ignore */ }
            }
        }
    }

    pub(crate) fn open(&mut self) -> Result<usize, AdapterError> {
        if let Some((index, socket)) = self
            .sockets
            .iter_mut()
            .enumerate()
            .find(|(_, e)| e.is_closed())
        {
            socket.state = SocketState::Open;
            return Ok(index);
        }

        Err(AdapterError::NoAvailableSockets)
    }

    pub(crate) fn close(&mut self, link_id: usize) -> Result<(), AdapterError> {
        let command = Command::CloseConnection(link_id);
        match self.send(command) {
            Ok(Response::Ok) | Ok(Response::UnlinkFail) => {
                self.sockets[link_id].state = SocketState::Closed;
                Ok(())
            }
            _ => Err(AdapterError::UnableToClose),
        }
    }

    pub(crate) fn connect_tcp(
        &mut self,
        link_id: usize,
        remote: HostSocketAddr,
    ) -> Result<(), AdapterError> {
        let command =
            Command::StartConnection(link_id, ConnectionType::TCP, remote.as_socket_addr());
        if let Ok(Response::Connect(..)) = self.send(command) {
            self.sockets[link_id].state = SocketState::Connected;
            return Ok(());
        }

        Err(AdapterError::UnableToOpen)
    }

    pub(crate) fn write(
        &mut self,
        link_id: usize,
        buffer: &[u8],
    ) -> nb::Result<usize, AdapterError> {
        self.process_notifications();

        let command = Command::Send {
            link_id,
            len: buffer.len(),
        };

        if let Ok(response) = self.send(command) {
            if let Response::Ok = response {
                if let Ok(response) = self.wait_for_response() {
                    if let Response::ReadyForData = response {
                        for b in buffer.iter() {
                            nb::block!(self.tx.write(*b))
                                .map_err(|_| nb::Error::from(AdapterError::WriteError))?;
                        }
                        let mut data_sent: Option<usize> = None;
                        loop {
                            match self.wait_for_response() {
                                Ok(Response::ReceivedDataToSend(len)) => {
                                    data_sent.replace(len);
                                }
                                Ok(Response::SendOk) => {
                                    return Ok(data_sent.unwrap_or_default());
                                }
                                _ => {
                                    break; // unknown response
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(nb::Error::from(AdapterError::WriteError))
    }

    pub(crate) fn read(
        &mut self,
        link_id: usize,
        buffer: &mut [u8],
    ) -> nb::Result<usize, AdapterError> {
        self.process_notifications();

        if matches!(self.sockets[link_id].state, SocketState::Closed) {
            return Err(nb::Error::Other(AdapterError::InvalidSocket));
        }

        if self.sockets[link_id].available == 0 {
            if matches!(self.sockets[link_id].state, SocketState::HalfClosed) {
                return Err(nb::Error::Other(AdapterError::InvalidSocket));
            } else {
                return Err(nb::Error::WouldBlock);
            }
        }

        let mut actual_len = buffer.len();
        if actual_len > crate::BUFFER_LEN {
            actual_len = crate::BUFFER_LEN;
        }

        let command = Command::Receive {
            link_id,
            len: actual_len,
        };

        match self.send(command) {
            Ok(Response::DataReceived(inbound, len)) => {
                for (i, b) in inbound[0..len].iter().enumerate() {
                    buffer[i] = *b;
                }
                self.sockets[link_id].available -= len;
                Ok(len)
            }
            Ok(Response::Ok) => Err(nb::Error::WouldBlock),
            _ => Err(nb::Error::Other(AdapterError::ReadError)),
        }
    }

    pub(crate) fn is_connected(&self, link_id: usize) -> Result<bool, AdapterError> {
        Ok(match self.sockets[link_id].state {
            SocketState::HalfClosed => self.sockets[link_id].available > 0,
            SocketState::Closed => false,
            SocketState::Open => false,
            SocketState::Connected => true,
        })
    }

    // ----------------------------------------------------------------------
    // DNS
    // ----------------------------------------------------------------------

    pub(crate) fn get_host_by_name(&mut self, hostname: &str) -> Result<HostAddr, DnsError> {
        let command = Command::GetHostByName { hostname };

        if let Ok(IpAddress(ip_addr)) = self.send(command) {
            Ok(HostAddr::new(ip_addr, Some(String::from(hostname))))
        } else {
            Err(DnsError::NoSuchHost)
        }
    }
}
