use crate::api::ip::IpAddress;
use crate::prelude::*;

pub enum TcpError {
    ReadError,
    WriteError,
    SocketClosed,
}

pub trait TcpStack: Actor {
    type SocketHandle: Copy;

    fn connect(self, dst: IpAddress) -> Response<Self, TcpSocket<Self>>;
    fn write(self, buf: &[u8]) -> Response<Self, Result<usize, TcpError>>;
    fn read(self, buf: &mut [u8]) -> Response<Self, Result<usize, TcpError>>;
    fn close(self, handle: Self::SocketHandle) -> Completion<Self>;
}

pub struct TcpSocket<S>
where
    S: TcpStack + 'static,
{
    handle: S::SocketHandle,
    stack: Address<S>,
}

impl<S> TcpSocket<S>
where
    S: TcpStack + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, TcpError> {
        self.stack.request_panicking(Write(buf)).await
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, TcpError> {
        self.stack.request_panicking(Read(buf)).await
    }

    fn close(self) {
        // consume/move self here, and allow to drop, triggering close
    }
}

pub struct Connect(IpAddress);

impl<S> RequestHandler<Connect> for S
where
    S: TcpStack + 'static,
{
    type Response = TcpSocket<S>;

    fn on_request(self, message: Connect) -> Response<Self, Self::Response> {
        self.connect(message.0)
    }
}

pub struct Write<'b>(&'b [u8]);

impl<'b, S> RequestHandler<Write<'b>> for S
where
    S: TcpStack,
{
    type Response = Result<usize, TcpError>;

    fn on_request(self, message: Write<'b>) -> Response<Self, Self::Response> {
        self.write(message.0)
    }
}

pub struct Read<'b>(&'b mut [u8]);

impl<'b, S> RequestHandler<Read<'b>> for S
where
    S: TcpStack,
{
    type Response = Result<usize, TcpError>;

    fn on_request(self, message: Read<'b>) -> Response<Self, Self::Response> {
        self.read(message.0)
    }
}

pub struct Close<S>(S::SocketHandle)
where
    S: TcpStack;

impl<S> NotifyHandler<Close<S>> for S
where
    S: TcpStack,
{
    fn on_notify(mut self, message: Close<S>) -> Completion<Self> {
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
    pub async fn tcp_connect(&self, dst: IpAddress) -> TcpSocket<S> {
        self.request(Connect(dst)).await
    }
}
