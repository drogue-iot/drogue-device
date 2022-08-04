use core::future::Future;
use embassy_net::{
    tcp::{ConnectError, Error as SocketError, TcpSocket},
    Device, IpAddress, Ipv4Address, Ipv6Address, Stack,
};
use embedded_io::Io;
use embedded_nal_async::{IpAddr, SocketAddr, TcpClientSocket};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    NotConnected,
    Connect(ConnectError),
    Socket(SocketError),
}

impl embedded_io::Error for Error {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl From<SocketError> for Error {
    fn from(e: SocketError) -> Self {
        Error::Socket(e)
    }
}

impl From<ConnectError> for Error {
    fn from(e: ConnectError) -> Self {
        Error::Connect(e)
    }
}

pub struct SmolTcpSocket<'a> {
    socket: Stack<'a>,
}

impl<'a> SmolTcpSocket<'a> {
    pub fn new<D: Device + 'a>(stack: &'a Stack<D>, tx: &'a mut [u8], rx: &'a mut [u8]) -> Self {
        Self {
            socket: TcpSocket::new(stack, rx, tx),
            connected: false,
        }
    }
}

impl<'a> Io for SmolTcpSocket<'a> {
    type Error = Error;
}

impl<'a> embedded_io::asynch::Read for SmolTcpSocket<'a> {
    type ReadFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move { Ok(self.socket.read(buf).await?) }
    }
}

impl<'a> embedded_io::asynch::Write for SmolTcpSocket<'a> {
    type WriteFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { Ok(self.socket.write(buf).await?) }
    }

    type FlushFuture<'m> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move { Ok(self.socket.flush().await?) }
    }
}

impl<'a> TcpClientSocket for SmolTcpSocket<'a> {
    type ConnectFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm
	where
		Self: 'm;

    fn connect<'m>(&'m mut self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            let addr: IpAddress = match remote.ip() {
                IpAddr::V4(addr) => IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.octets())),
                IpAddr::V6(addr) => IpAddress::Ipv6(Ipv6Address::from_bytes(&addr.octets())),
            };
            let remote_endpoint = (addr, remote.port());
            self.socket.connect(remote_endpoint).await?;
            self.connected = true;
            Ok(())
        }
    }

    type IsConnectedFuture<'m> = impl Future<Output = Result<bool, Self::Error>> + 'm
    where
        Self: 'm;
    fn is_connected<'m>(&'m mut self) -> Self::IsConnectedFuture<'m> {
        async move { Ok(self.connected) }
    }

    fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.socket.close();
        self.connected = false;
        Ok(())
    }
}
