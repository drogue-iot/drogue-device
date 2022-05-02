use super::socket_pool::PoolHandle;
use super::socket_pool::SocketPool;
use crate::network::tcp::TcpError;
use core::future::Future;
use core::marker::PhantomData;
use embassy::io::{AsyncBufReadExt, AsyncWriteExt};
use embassy_net::Ipv4Address;
use embedded_nal_async::*;

pub struct SmolTcpStack<
    'buffer,
    const POOL_SIZE: usize,
    const BACKLOG: usize,
    const BUF_SIZE: usize,
> {
    buffer_pool: SocketPool<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>,
    _marker: PhantomData<&'buffer ()>,
}

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize>
    SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    pub fn new() -> Self {
        let me = Self {
            buffer_pool: SocketPool::new(),
            _marker: PhantomData,
        };
        me.buffer_pool.initialize();
        me
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SmolSocketHandle(PoolHandle);

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> TcpClientStack
    for SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type TcpSocket = SmolSocketHandle;
    type Error = TcpError;

    type SocketFuture<'m> = impl Future<Output = Result<Self::TcpSocket, Self::Error>> + 'm
    where
        Self: 'm;
    fn socket<'m>(&'m mut self) -> Self::SocketFuture<'m> {
        async move {
            let handle = self
                .buffer_pool
                .borrow()
                .await
                .map_err(|_| TcpError::OpenError)?;
            Ok(SmolSocketHandle(handle))
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
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            match remote.ip() {
                IpAddr::V4(addr) => {
                    let [a, b, c, d] = addr.octets();
                    let remote_addr = Ipv4Address::new(a, b, c, d);
                    let remote_endpoint = (remote_addr, remote.port());
                    socket
                        .connect(remote_endpoint)
                        .await
                        .map_err(|_| TcpError::ConnectError)
                }
                _ => Err(TcpError::ConnectError),
            }
        }
    }

    type IsConnectedFuture<'m> =
        impl Future<Output = Result<bool, Self::Error>> + 'm where Self: 'm;
    fn is_connected<'m>(&'m mut self, _handle: &'m Self::TcpSocket) -> Self::IsConnectedFuture<'m> {
        async move { todo!() }
    }

    type SendFuture<'m> =
        impl Future<Output = Result<usize, Self::Error>> + 'm where Self: 'm;
    fn send<'m>(
        &'m mut self,
        handle: &'m mut Self::TcpSocket,
        buf: &'m [u8],
    ) -> Self::SendFuture<'m> {
        async move {
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            socket.write(buf).await.map_err(|_| TcpError::WriteError)
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
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            socket.read(buf).await.map_err(|_| TcpError::ReadError)
        }
    }

    type CloseFuture<'m> =
        impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn close<'m>(&'m mut self, handle: Self::TcpSocket) -> Self::CloseFuture<'m> {
        async move {
            self.buffer_pool.unborrow(handle.0);
            Ok(())
        }
    }
}
