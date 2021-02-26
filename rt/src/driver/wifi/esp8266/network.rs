use crate::adapter::{Adapter, AdapterError};
use embedded_hal::serial::Write;

use core::cell::RefCell;
use drogue_network::addr::{
    HostAddr,
    HostSocketAddr,
    IpAddr
};
use drogue_network::tcp::{
    Mode,
    TcpStack,
    TcpError,
    TcpImplError,
};
use core::fmt::Debug;
use nom::lib::std::fmt::Formatter;
use heapless::{
    String,
    consts::{
        U256,
    },
};
use drogue_network::IpNetworkDriver;
use drogue_network::dns::{Dns, DnsError, AddrType};
/// Network driver based on the ESP8266 board
pub struct Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    adapter: RefCell<Adapter<'a, Tx>>,
}


impl<'a, Tx> Debug for Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple( "Esp8266IpNetworkDriver").finish()
    }
}

impl<'a, Tx> Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    pub(crate) fn new(adapter: Adapter<'a, Tx>) -> Self {
        Self {
            adapter: RefCell::new(adapter),
        }
    }
}

impl<'a, Tx> IpNetworkDriver for Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    type TcpSocket = TcpSocket;
    type TcpError = TcpError;
    type DnsError = DnsError;

    fn tcp(&self) -> &dyn TcpStack<TcpSocket=Self::TcpSocket, Error=Self::TcpError> {
        self as &dyn TcpStack<TcpSocket = Self::TcpSocket, Error = Self::TcpError>
    }

    fn dns(&self) -> &dyn Dns<Error=Self::DnsError> {
        self as &dyn Dns<Error = Self::DnsError>
    }
}

/// Handle to a socket.
pub struct TcpSocket {
    link_id: usize,
    mode: Mode,
}

impl Debug for TcpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TcpSocket")
            .field("link_id", &self.link_id)
            .field("mode",
                   &match self.mode {
                       Mode::Blocking => {
                           "blocking"
                       }
                       Mode::NonBlocking => {
                           "non-blocking"
                       }
                       Mode::Timeout(_t) => {
                           "timeout"
                       }
                   },
            )
            .finish()
    }
}

/*
impl Into<TcpError> for AdapterError {
    fn into(self) -> TcpError {
        match self {
            AdapterError::Timeout => {
                TcpError::Timeout
            },
            AdapterError::WriteError => {
                TcpError::WriteError
            },
            AdapterError::InvalidSocket => {
                TcpError::SocketNotOpen
            },
            _ => {
                TcpError::Impl(TcpImplError::Unknown)
            },
        }
    }
}
 */

impl From<AdapterError> for TcpError {
    fn from(error: AdapterError) -> Self {
        match error {
            AdapterError::Timeout => {
                TcpError::Timeout
            }
            AdapterError::WriteError => {
                TcpError::WriteError
            }
            AdapterError::ReadError => {
                TcpError::ReadError
            }
            AdapterError::InvalidSocket => {
                TcpError::SocketNotOpen
            }
            _ => {
                TcpError::Impl(TcpImplError::Unknown)
            }
        }
    }
}

impl<'a, Tx> TcpStack for Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    type TcpSocket = TcpSocket;
    type Error = TcpError;

    fn open(&self, mode: Mode) -> Result<Self::TcpSocket, Self::Error> {
        let mut adapter = self.adapter.borrow_mut();
        Ok(TcpSocket {
            link_id: adapter.open()?,
            mode,
        })
    }

    fn connect(
        &self,
        socket: Self::TcpSocket,
        remote: HostSocketAddr,
    ) -> Result<Self::TcpSocket, Self::Error> {
        let mut adapter = self.adapter.borrow_mut();

        adapter.connect_tcp(socket.link_id, remote)?;
        Ok(socket)
    }

    fn is_connected(&self, socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        let adapter = self.adapter.borrow();
        adapter.is_connected(socket.link_id).map_err(TcpError::from)
    }

    fn write(&self, socket: &mut Self::TcpSocket, buffer: &[u8]) -> nb::Result<usize, Self::Error> {
        let mut adapter = self.adapter.borrow_mut();

        Ok(adapter
            .write(socket.link_id, buffer)
            .map_err(|e| { e.map(TcpError::from) })?)
    }

    fn read(
        &self,
        socket: &mut Self::TcpSocket,
        buffer: &mut [u8],
    ) -> nb::Result<usize, Self::Error> {
        let mut adapter = self.adapter.borrow_mut();

        match socket.mode {
            Mode::Blocking => {
                nb::block!(
                adapter.read(socket.link_id, buffer))
                    .map_err(|e|
                        nb::Error::from(TcpError::from(e))
                    )
            }
            Mode::NonBlocking => {
                adapter.read(socket.link_id, buffer)
                    .map_err(|e|
                        e.map(TcpError::from)
                    )
            }
            Mode::Timeout(_) => unimplemented!(),
        }
    }

    fn close(&self, socket: Self::TcpSocket) -> Result<(), Self::Error> {
        let mut adapter = self.adapter.borrow_mut();
        adapter.close(socket.link_id).map_err(|e| e.into())
    }
}

impl<'a, Tx> Dns for Esp8266IpNetworkDriver<'a, Tx>
    where
        Tx: Write<u8>,
{
    type Error = DnsError;

    fn gethostbyname(&self, hostname: &str, addr_type: AddrType) -> Result<HostAddr, Self::Error> {
        match addr_type {
            AddrType::IPv6 => {
                Err(DnsError::UnsupportedAddressType)
            },
            _ => {
                let mut adapter = self.adapter.borrow_mut();
                adapter.get_host_by_name(hostname)
            }
        }
    }

    fn gethostbyaddr(&self, _addr: IpAddr) -> Result<String<U256>, Self::Error> {
        unimplemented!()
    }
}

