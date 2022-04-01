use super::socket_pool::PoolHandle;
use super::socket_pool::SocketPool;
use crate::traits::ip::{IpAddress, IpProtocol, SocketAddress};
use crate::traits::tcp::{TcpError, TcpStack};
use core::future::Future;
use core::marker::PhantomData;
use embassy::io::{AsyncBufReadExt, AsyncWriteExt};
use embassy_net::Ipv4Address;

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

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> TcpStack
    for SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type SocketHandle = SmolSocketHandle;

    type OpenFuture<'m> = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm
    where
        'buffer: 'm;

    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            let handle = self
                .buffer_pool
                .borrow()
                .await
                .map_err(|_| TcpError::OpenError)?;
            Ok(SmolSocketHandle(handle))
        }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        'buffer: 'm;

    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            match proto {
                IpProtocol::Tcp => {
                    let socket = self
                        .buffer_pool
                        .get_socket(handle.0)
                        .map_err(|_| TcpError::WriteError)?;
                    match dst.ip() {
                        IpAddress::V4(addr) => {
                            let [a, b, c, d] = addr.octets();
                            let remote_addr = Ipv4Address::new(a, b, c, d);
                            let remote_endpoint = (remote_addr, dst.port());
                            socket
                                .connect(remote_endpoint)
                                .await
                                .map_err(|_| TcpError::ConnectError)
                        }
                    }
                }
                IpProtocol::Udp => Err(TcpError::ConnectError),
            }
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        'buffer: 'm;

    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            socket.write(buf).await.map_err(|_| TcpError::WriteError)
        }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        'buffer: 'm;

    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            socket.read(buf).await.map_err(|_| TcpError::ReadError)
        }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        'buffer: 'm;

    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.buffer_pool.unborrow(handle.0);
            Ok(())
        }
    }
}
