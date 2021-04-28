use super::ip::{IpProtocol, SocketAddress};
use core::future::Future;

#[derive(Debug)]
pub enum TcpError {
    ConnectError,
    ReadError,
    WriteError,
    CloseError,
    SocketClosed,
}

pub trait TcpStack {
    type SocketHandle: Copy;

    type OpenFuture<'m>: Future<Output = Self::SocketHandle>
    where
        Self: 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m>;

    type ConnectFuture<'m>: Future<Output = Result<(), TcpError>>
    where
        Self: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m>;

    type WriteFuture<'m>: Future<Output = Result<usize, TcpError>>
    where
        Self: 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m>;

    type ReadFuture<'m>: Future<Output = Result<usize, TcpError>>
    where
        Self: 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m>;

    type CloseFuture<'m>: Future<Output = ()>
    where
        Self: 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m>;
}
