use crate::shared::*;
use crate::traits::{ip::*, tcp::*};
use core::future::Future;

pub type TcpStackState<T> = Shared<T>;
pub type SharedTcpStack<'a, T> = Handle<'a, T>;

impl<'a, T: TcpStack> TcpStack for Handle<'a, T> {
    type SocketHandle = T::SocketHandle;

    type OpenFuture<'m> = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm
    where
        Self: 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move { self.lock().await.open().await }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move { self.lock().await.connect(handle, proto, dst).await }
    }

    type WriteFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { self.lock().await.write(handle, buf).await }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move { self.lock().await.read(handle, buf).await }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move { self.lock().await.close(handle).await }
    }
}
