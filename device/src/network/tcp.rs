use crate::shared::*;
use core::future::Future;
use embedded_nal_async::*;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TcpError {
    OpenError,
    ConnectError,
    ReadError,
    WriteError,
    CloseError,
    IoError,
    SocketClosed,
}

pub type TcpStackState<T> = Shared<T>;
pub type SharedTcpStack<'a, T> = Handle<'a, T>;

impl<'a, T: TcpClientStack> TcpClientStack for Handle<'a, T> {
    type TcpSocket = T::TcpSocket;
    type Error = T::Error;

    type SocketFuture<'m> = impl Future<Output = Result<Self::TcpSocket, T::Error>> + 'm
    where
        Self: 'm;
    fn socket<'m>(&'m mut self) -> Self::SocketFuture<'m> {
        async move { self.lock().await.socket().await }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), T::Error>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn connect<'m>(
        &'m mut self,
        socket: &'m mut Self::TcpSocket,
        remote: SocketAddr,
    ) -> Self::ConnectFuture<'m> {
        async move { self.lock().await.connect(socket, remote).await }
    }

    type IsConnectedFuture<'m> =
        impl Future<Output = Result<bool, T::Error>> + 'm where Self: 'm;
    fn is_connected<'m>(&'m mut self, socket: &'m Self::TcpSocket) -> Self::IsConnectedFuture<'m> {
        async move { self.lock().await.is_connected(socket).await }
    }

    type SendFuture<'m> =
        impl Future<Output = Result<usize, T::Error>> + 'm where Self: 'm;
    fn send<'m>(
        &'m mut self,
        socket: &'m mut Self::TcpSocket,
        buffer: &'m [u8],
    ) -> Self::SendFuture<'m> {
        async move { self.lock().await.send(socket, buffer).await }
    }

    type ReceiveFuture<'m> =
        impl Future<Output = Result<usize, T::Error>> + 'm where Self: 'm;
    fn receive<'m>(
        &'m mut self,
        socket: &'m mut Self::TcpSocket,
        buffer: &'m mut [u8],
    ) -> Self::ReceiveFuture<'m> {
        async move { self.lock().await.receive(socket, buffer).await }
    }

    type CloseFuture<'m> =
        impl Future<Output = Result<(), T::Error>> + 'm where Self: 'm;
    fn close<'m>(&'m mut self, socket: Self::TcpSocket) -> Self::CloseFuture<'m> {
        async move { self.lock().await.close(socket).await }
    }
}
