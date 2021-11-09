use crate::traits::ip::{IpAddress, IpProtocol, SocketAddress};
use crate::traits::tcp::{TcpError, TcpStack};
use core::future::Future;
use core::marker::PhantomData;
use embassy::io::{AsyncBufReadExt, AsyncWriteExt};
use embassy_net::Ipv4Address;
use socket_pool::PoolHandle;
use socket_pool::SocketPool;

mod socket_pool;

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
        Self {
            buffer_pool: SocketPool::new(),
            _marker: PhantomData,
        }
    }

    pub(crate) unsafe fn initialize(&self) {
        self.buffer_pool.initialize()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SmolSocketHandle(PoolHandle);

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> TcpStack
    for SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type SocketHandle = SmolSocketHandle;

    #[rustfmt::skip]
    type OpenFuture<'m> where 'buffer: 'm = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm;

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

    #[rustfmt::skip]
    type ConnectFuture<'m> where 'buffer: 'm = impl Future<Output = Result<(), TcpError>> + 'm;

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

    #[rustfmt::skip]
    type WriteFuture<'m> where 'buffer: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;

    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            let socket = self
                .buffer_pool
                .get_socket(handle.0)
                .map_err(|_| TcpError::WriteError)?;
            socket.write(buf).await.map_err(|_| TcpError::WriteError)
        }
    }

    #[rustfmt::skip]
    type ReadFuture<'m> where 'buffer: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;

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

    #[rustfmt::skip]
    type CloseFuture<'m> where 'buffer: 'm = impl Future<Output = Result<(), TcpError>> + 'm;

    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.buffer_pool.unborrow(handle.0);
            Ok(())
        }
    }
}
