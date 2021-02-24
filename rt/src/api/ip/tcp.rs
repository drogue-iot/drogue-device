use crate::api::ip::{IpProtocol, SocketAddress};
use crate::prelude::*;

pub enum TcpError {
    ConnectError,
    ReadError,
    WriteError,
    CloseError,
    SocketClosed,
}

pub trait TcpStack: Actor {
    type SocketHandle: Copy;

    fn open(self) -> Response<Self, Self::SocketHandle>;
    fn connect(
        self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Response<Self, Result<(), TcpError>>;
    fn write(
        self,
        handle: Self::SocketHandle,
        buf: &[u8],
    ) -> Response<Self, Result<usize, TcpError>>;
    fn read(
        self,
        handle: Self::SocketHandle,
        buf: &mut [u8],
    ) -> Response<Self, Result<usize, TcpError>>;
    fn close(self, handle: Self::SocketHandle) -> Completion<Self>;
}

pub struct TcpSocket<S>
where
    S: TcpStack + 'static,
{
    stack: Address<S>,
    handle: S::SocketHandle,
}

impl<S> TcpSocket<S>
where
    S: TcpStack + 'static,
{
    pub(crate) fn new(stack: Address<S>, handle: S::SocketHandle) -> Self {
        Self { handle, stack }
    }

    pub async fn connect(
        &mut self,
        proto: IpProtocol,
        addr: SocketAddress,
    ) -> Result<(), TcpError> {
        self.stack.request(Connect(self.handle, proto, addr)).await
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<usize, TcpError> {
        self.stack.request_panicking(Write(self.handle, buf)).await
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, TcpError> {
        self.stack.request_panicking(Read(self.handle, buf)).await
    }

    pub fn close(self) {
        // consume/move self here, and allow to drop, triggering close
    }
}

pub struct Open;

impl<S> RequestHandler<Open> for S
where
    S: TcpStack + 'static,
{
    type Response = S::SocketHandle;

    fn on_request(self, message: Open) -> Response<Self, Self::Response> {
        self.open()
    }
}

pub struct Connect<S>(S::SocketHandle, IpProtocol, SocketAddress)
where
    S: TcpStack;

impl<S> RequestHandler<Connect<S>> for S
where
    S: TcpStack + 'static,
{
    type Response = Result<(), TcpError>;

    fn on_request(self, message: Connect<S>) -> Response<Self, Self::Response> {
        self.connect(message.0, message.1, message.2)
    }
}

pub struct Write<'b, S>(S::SocketHandle, &'b [u8])
where
    S: TcpStack;

impl<'b, S> RequestHandler<Write<'b, S>> for S
where
    S: TcpStack,
{
    type Response = Result<usize, TcpError>;

    fn on_request(self, message: Write<'b, S>) -> Response<Self, Self::Response> {
        self.write(message.0, message.1)
    }
}

pub struct Read<'b, S>(S::SocketHandle, &'b mut [u8])
where
    S: TcpStack;

impl<'b, S> RequestHandler<Read<'b, S>> for S
where
    S: TcpStack,
{
    type Response = Result<usize, TcpError>;

    fn on_request(self, message: Read<'b, S>) -> Response<Self, Self::Response> {
        self.read(message.0, message.1)
    }
}

pub struct Close<S>(S::SocketHandle)
where
    S: TcpStack;

impl<S> NotifyHandler<Close<S>> for S
where
    S: TcpStack,
{
    fn on_notify(self, message: Close<S>) -> Completion<Self> {
        self.close(message.0)
    }
}

impl<S> Drop for TcpSocket<S>
where
    S: TcpStack,
{
    fn drop(&mut self) {
        self.stack.notify(Close(self.handle));
    }
}

impl<S> Address<S>
where
    S: TcpStack + 'static,
{
    pub async fn tcp_open(&self) -> TcpSocket<S> {
        let handle = self.request(Open).await;
        TcpSocket::new(*self, handle)
    }
}
