use crate::BUFFER_LEN;
use core::fmt;
use core::fmt::{Debug, Write};
use drogue_network::ip::{IpAddress, IpAddressV4, SocketAddress};
use heapless::{consts::U256, String};

#[derive(Debug)]
pub struct ResolverAddresses {
    pub resolver1: IpAddressV4,
    pub resolver2: Option<IpAddressV4>,
}

/// Type of socket connection.
#[derive(Debug)]
pub enum ConnectionType {
    TCP,
    UDP,
}

/// Mode of the Wi-Fi stack
#[derive(Debug)]
pub enum WiFiMode {
    /// Station mode, aka client
    Station,
    /// Access point mode
    SoftAccessPoint,
    /// Access point + station mode
    SoftAccessPointAndStation,
}

/// Commands to be sent to the ESP board.
#[derive(Debug)]
pub enum Command<'a> {
    QueryFirmwareInfo,
    SetMode(WiFiMode),
    JoinAp { ssid: &'a str, password: &'a str },
    QueryIpAddress,
    StartConnection(usize, ConnectionType, SocketAddress),
    CloseConnection(usize),
    Send { link_id: usize, len: usize },
    Receive { link_id: usize, len: usize },
    QueryDnsResolvers,
    SetDnsResolvers(ResolverAddresses),
    GetHostByName { hostname: &'a str },
}

impl<'a> Command<'a> {
    pub fn as_bytes(&self) -> String<U256> {
        match self {
            Command::QueryFirmwareInfo => String::from("AT+GMR"),
            Command::QueryIpAddress => String::from("AT+CIPSTA_CUR?"),
            Command::SetMode(mode) => match mode {
                WiFiMode::Station => String::from("AT+CWMODE_CUR=1"),
                WiFiMode::SoftAccessPoint => String::from("AT+CWMODE_CUR=2"),
                WiFiMode::SoftAccessPointAndStation => String::from("AT+CWMODE_CUR=3"),
            },
            Command::JoinAp { ssid, password } => {
                let mut s = String::from("AT+CWJAP_CUR=\"");
                s.push_str(ssid).unwrap();
                s.push_str("\",\"").unwrap();
                s.push_str(password).unwrap();
                s.push_str("\"").unwrap();
                s
            }
            Command::StartConnection(link_id, connection_type, socket_addr) => {
                let mut s = String::from("AT+CIPSTART=");
                write!(s, "{},", link_id).unwrap();
                match connection_type {
                    ConnectionType::TCP => {
                        write!(s, "\"TCP\"").unwrap();
                    }
                    ConnectionType::UDP => {
                        write!(s, "\"UDP\"").unwrap();
                    }
                }
                write!(s, ",").unwrap();
                match socket_addr.ip() {
                    IpAddress::V4(ip) => {
                        write!(s, "\"{}\",{}", ip, socket_addr.port()).unwrap();
                    } //IpAddress::V6(_) => panic!("IPv6 not supported"),
                }
                s as String<U256>
            }
            Command::CloseConnection(link_id) => {
                let mut s = String::from("AT+CIPCLOSE=");
                write!(s, "{}", link_id).unwrap();
                s
            }
            Command::Send { link_id, len } => {
                let mut s = String::from("AT+CIPSEND=");
                write!(s, "{},{}", link_id, len).unwrap();
                s
            }
            Command::Receive { link_id, len } => {
                let mut s = String::from("AT+CIPRECVDATA=");
                write!(s, "{},{}", link_id, len).unwrap();
                s
            }
            Command::QueryDnsResolvers => String::from("AT+CIPDNS_CUR?"),
            Command::SetDnsResolvers(addr) => {
                let mut s = String::from("AT+CIPDNS_CUR=1,");
                write!(s, "\"{}\"", addr.resolver1).unwrap();
                if let Some(resolver2) = addr.resolver2 {
                    write!(s, ",\"{}\"", resolver2).unwrap()
                }
                s
            }
            Command::GetHostByName { hostname } => {
                let mut s = String::from("AT+CIPDOMAIN=");
                write!(s, "\"{}\"", hostname).unwrap();
                s
            }
        }
    }
}

/// Responses (including unsolicited) which may be parsed from the board.
#[allow(clippy::large_enum_variant)]
pub enum Response {
    None,
    Ok,
    Error,
    FirmwareInfo(FirmwareInfo),
    ReadyForData,
    ReceivedDataToSend(usize),
    SendOk,
    SendFail,
    DataAvailable { link_id: usize, len: usize },
    DataReceived([u8; BUFFER_LEN], usize),
    WifiConnected,
    WifiConnectionFailure(WifiConnectionFailure),
    WifiDisconnect,
    GotIp,
    IpAddresses(IpAddresses),
    Connect(usize),
    Closed(usize),
    Resolvers(ResolverAddresses),
    IpAddress(IpAddress),
    DnsFail,
    UnlinkFail,
}

impl Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Response::None => f.write_str("None"),
            Response::Ok => f.write_str("Ok"),
            Response::Error => f.write_str("Error"),
            Response::FirmwareInfo(v) => f.debug_tuple("FirmwareInfo").field(v).finish(),
            Response::ReadyForData => f.write_str("ReadyForData"),
            Response::ReceivedDataToSend(len) => {
                f.debug_tuple("ReceivedDataToSend").field(len).finish()
            }
            Response::SendOk => f.write_str("SendOk"),
            Response::SendFail => f.write_str("SendFail"),
            Response::DataAvailable { link_id, len } => f
                .debug_struct("DataAvailable")
                .field("link_id", link_id)
                .field("len", len)
                .finish(),
            //Response::DataReceived(d, l) => dump_data("DataReceived", d, *l, f),
            Response::DataReceived(_, _) => f.write_str("DataReceived"),
            Response::WifiConnected => f.write_str("WifiConnected"),
            Response::WifiConnectionFailure(v) => {
                f.debug_tuple("WifiConnectionFailure").field(v).finish()
            }
            Response::WifiDisconnect => f.write_str("WifiDisconnect"),
            Response::GotIp => f.write_str("GotIp"),
            Response::IpAddresses(v) => f.debug_tuple("IpAddresses").field(v).finish(),
            Response::Connect(v) => f.debug_tuple("Connect").field(v).finish(),
            Response::Closed(v) => f.debug_tuple("Closed").field(v).finish(),
            Response::IpAddress(v) => f.debug_tuple("IpAddress").field(v).finish(),
            Response::Resolvers(v) => f.debug_tuple("Resolvers").field(v).finish(),
            Response::DnsFail => f.write_str("DNS Fail"),
            Response::UnlinkFail => f.write_str("UnlinkFail"),
        }
    }
}

/// IP addresses for the board, including its own address, netmask and gateway.
#[derive(Debug)]
pub struct IpAddresses {
    pub ip: IpAddressV4,
    pub gateway: IpAddressV4,
    pub netmask: IpAddressV4,
}

/// Version information for the ESP board.
#[derive(Debug)]
pub struct FirmwareInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

/// Reasons for Wifi access-point join failures.
#[derive(Debug)]
pub enum WifiConnectionFailure {
    Timeout,
    WrongPassword,
    CannotFindTargetAp,
    ConnectionFailed,
}

impl From<u8> for WifiConnectionFailure {
    fn from(code: u8) -> Self {
        match code {
            1 => WifiConnectionFailure::Timeout,
            2 => WifiConnectionFailure::WrongPassword,
            3 => WifiConnectionFailure::CannotFindTargetAp,
            _ => WifiConnectionFailure::ConnectionFailed,
        }
    }
}

/// Dump some data, which is stored in a buffer with a length indicator.
///
/// The output will contain the field name, the data as string (only 7bits) and the raw bytes
/// in hex encoding.
#[allow(dead_code)]
fn dump_data(name: &str, data: &[u8], len: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let data = &data[0..len];

    f.write_str(name)?;
    f.write_char('(')?;

    f.write_fmt(format_args!("{}; '", len))?;

    for d in data {
        if *d == 0 {
            f.write_str("\\0")?;
        } else if *d <= 0x7F {
            f.write_char(*d as char)?;
        } else {
            f.write_char('\u{FFFD}')?;
        }
    }

    f.write_str("'; ")?;
    f.write_fmt(format_args!("{:X?}", data))?;
    f.write_char(')')?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use arrayvec::ArrayString;
    use core::fmt::Write;

    #[test]
    fn test_debug_no_value() {
        let mut buf = ArrayString::<[u8; 20]>::new();

        write!(&mut buf, "{:?}", Response::Ok).expect("Can't write");
        assert_eq!(&buf, "Ok");
    }

    #[test]
    fn test_debug_simple_value() {
        let mut buf = ArrayString::<[u8; 20]>::new();

        write!(&mut buf, "{:?}", Response::Connect(1)).expect("Can't write");
        assert_eq!(&buf, "Connect(1)");
    }

    fn test_debug_data() {
        let mut buf = ArrayString::<[u8; 256]>::new();
        let data = b"FOO\0BAR";

        let mut array = [0u8; super::BUFFER_LEN];
        for (&x, p) in data.iter().zip(array.iter_mut()) {
            *p = x;
        }

        write!(&mut buf, "{:?}", Response::DataReceived(array, data.len())).expect("Can't write");
        assert_eq!(
            &buf,
            "DataReceived(7; 'FOO\\0BAR'; [46, 4F, 4F, 0, 42, 41, 52])"
        );
    }
}
