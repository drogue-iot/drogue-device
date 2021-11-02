#[cfg(feature = "tcp+smoltcp")]
pub mod smoltcp;

use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::{TcpError, TcpSocket, TcpStack},
    },
};

use core::future::Future;

use core::convert::TryInto;
// Trait that defines the mapping from API to request and response to result.
pub trait TcpActor<A>
where
    A: Actor,
    A::Response: Into<TcpResponse<Self::SocketHandle>>,
{
    type SocketHandle: Copy;
    fn open<'m>() -> A::Message<'m>;
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> A::Message<'m>;
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> A::Message<'m>;
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> A::Message<'m>;
    fn close<'m>(handle: Self::SocketHandle) -> A::Message<'m>;
}

// Response type that TcpActors must be able to produce form their Response message.
pub enum TcpResponse<H> {
    Open(Result<H, TcpError>),
    Connect(Result<(), TcpError>),
    Write(Result<usize, TcpError>),
    Read(Result<usize, TcpError>),
    Close,
}

impl<H> TcpResponse<H> {
    pub fn open(self) -> Result<H, TcpError> {
        match self {
            TcpResponse::Open(Ok(handle)) => Ok(handle),
            _ => Err(TcpError::OpenError),
        }
    }

    pub fn connect(self) -> Result<(), TcpError> {
        match self {
            TcpResponse::Connect(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    pub fn write(self) -> Result<usize, TcpError> {
        match self {
            TcpResponse::Write(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    pub fn read(self) -> Result<usize, TcpError> {
        match self {
            TcpResponse::Read(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    pub fn close(self) {
        match self {
            TcpResponse::Close => (),
            _ => panic!("unexpected response type"),
        }
    }
}
