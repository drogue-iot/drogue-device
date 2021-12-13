#[cfg(feature = "tcp+smoltcp")]
pub mod smoltcp;

#[cfg(feature = "std")]
pub mod std;

use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::TcpError,
    },
};

// Trait that defines the mapping from API to request and response to result.
pub trait TcpActor: Actor {
    type SocketHandle: Copy;
    fn open<'m>() -> Self::Message<'m>;
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::Message<'m>;
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> Self::Message<'m>;
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> Self::Message<'m>;
    fn close<'m>(handle: Self::SocketHandle) -> Self::Message<'m>;

    fn into_response(response: Self::Response) -> Option<TcpResponse<Self::SocketHandle>>;
}

// Response type that TcpActors must be able to produce form their Response message.
pub enum TcpResponse<H> {
    Open(Result<H, TcpError>),
    Connect(Result<(), TcpError>),
    Write(Result<usize, TcpError>),
    Read(Result<usize, TcpError>),
    Close(Result<(), TcpError>),
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

    pub fn close(self) -> Result<(), TcpError> {
        match self {
            TcpResponse::Close(result) => result,
            _ => panic!("unexpected response type"),
        }
    }
}

pub enum TcpRequest<'m, H> {
    Open,
    Connect(H, IpProtocol, SocketAddress),
    Write(H, &'m [u8]),
    Read(H, &'m mut [u8]),
    Close(H),
}

impl<'a, T> Address<T>
where
    T: TcpActor + 'a,
{
    pub async fn open(&self) -> Result<T::SocketHandle, TcpError> {
        let m = T::open();
        T::into_response(Address::request(self, m).unwrap().await)
            .unwrap()
            .open()
    }
}
