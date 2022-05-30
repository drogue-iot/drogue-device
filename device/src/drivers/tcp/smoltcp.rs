use core::future::Future;
use embassy_net::{
    tcp::{ConnectError, TcpSocket},
    Device, IpAddress, Ipv4Address, Ipv6Address, Stack,
};
use embedded_io::Io;
use embedded_nal_async::{IpAddr, SocketAddr, TcpClient};

pub struct SmolTcpClient<'a, D, const BUF_SIZE: usize>
where
    D: Device,
{
    tx: [u8; BUF_SIZE],
    rx: [u8; BUF_SIZE],
    stack: &'a Stack<D>,
}

impl<'a, D, const BUF_SIZE: usize> SmolTcpClient<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    pub fn new(stack: &'a Stack<D>) -> Self {
        Self {
            tx: [0; BUF_SIZE],
            rx: [0; BUF_SIZE],
            stack,
        }
    }
}

impl<'a, D, const BUF_SIZE: usize> Io for SmolTcpClient<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type Error = ConnectError;
}
impl<'a, D, const BUF_SIZE: usize> TcpClient for SmolTcpClient<'a, D, BUF_SIZE>
where
    D: Device + 'a,
{
    type TcpConnection<'m> = TcpSocket<'m> where Self: 'm;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::TcpConnection<'m>, Self::Error>> + 'm
	where
		Self: 'm;

    fn connect<'m>(&'m mut self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            let mut socket = TcpSocket::new(&self.stack, &mut self.rx, &mut self.tx);
            let addr: IpAddress = match remote.ip() {
                IpAddr::V4(addr) => IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.octets())),
                IpAddr::V6(addr) => IpAddress::Ipv6(Ipv6Address::from_bytes(&addr.octets())),
            };
            let remote_endpoint = (addr, remote.port());
            socket.connect(remote_endpoint).await?;
            Ok(socket)
        }
    }
}
