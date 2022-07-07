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

pub struct SmolTcpSocket<'a, D, const BUF_SIZE: usize>
where
    D: Device,
{
    tx: [u8; BUF_SIZE],
    rx: [u8; BUF_SIZE],
    stack: &'a Stack<D>,
    socket: Option<TcpSocket<'a>>,
}

impl<'a, D, const BUF_SIZE: usize> SmolTcpSocket<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    pub fn new(stack: &'a Stack<D>) -> Self {
        Self {
            tx: [0; BUF_SIZE],
            rx: [0; BUF_SIZE],
            stack,
            socket: None,
        }
    }
}

impl<'a, D, const BUF_SIZE: usize> Io for SmolTcpSocket<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type Error = Error;
}

impl<'a, D, const BUF_SIZE: usize> embedded_io::asynch::Read for SmolTcpSocket<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type ReadFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            if let Some(socket) = &mut self.socket {
                Ok(socket.read(buf).await?)
            } else {
                Err(Error::NotConnected)
            }
        }
    }
}

impl<'a, D, const BUF_SIZE: usize> embedded_io::asynch::Write for SmolTcpSocket<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type WriteFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            if let Some(socket) = &mut self.socket {
                Ok(socket.write(buf).await?)
            } else {
                Err(Error::NotConnected)
            }
        }
    }

    type FlushFuture<'m> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move {
            if let Some(socket) = &mut self.socket {
                Ok(socket.flush().await?)
            } else {
                Err(Error::NotConnected)
            }
        }
    }
}

impl<'a, D, const BUF_SIZE: usize> TcpClientSocket for SmolTcpSocket<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type ConnectFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm
	where
		Self: 'm;

    fn connect<'m>(&'m mut self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            if self.socket.is_some() {
                let _ = self.socket.take();
            }
            let mut socket = TcpSocket::new(&self.stack, &mut self.rx, &mut self.tx);
            let addr: IpAddress = match remote.ip() {
                IpAddr::V4(addr) => IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.octets())),
                IpAddr::V6(addr) => IpAddress::Ipv6(Ipv6Address::from_bytes(&addr.octets())),
            };
            let remote_endpoint = (addr, remote.port());
            socket.connect(remote_endpoint).await?;
            self.socket.replace(socket);
            Ok(())
        }
    }

    type IsConnectedFuture<'m> = impl Future<Output = Result<bool, Self::Error>> + 'm
    where
        Self: 'm;
    fn is_connected<'m>(&'m mut self) -> Self::IsConnectedFuture<'m> {
        async move { Ok(self.socket.is_some()) }
    }

    fn disconnect(&mut self) -> Result<(), Self::Error> {
        let _ = self.socket.take();
        Ok(())
    }
}
