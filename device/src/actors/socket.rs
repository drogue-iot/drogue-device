use super::tcp::TcpActor;
use crate::traits::{
    ip::{IpProtocol, SocketAddress},
    tcp::TcpError,
};
use crate::Address;

/// A Socket type for connecting to a network endpoint + sending and receiving data.
#[derive(Clone, Copy)]
pub struct Socket<A>
where
    A: TcpActor + 'static,
{
    address: Address<A>,
    handle: A::SocketHandle,
}

impl<A> Socket<A>
where
    A: TcpActor + 'static,
{
    pub fn new(address: Address<A>, handle: A::SocketHandle) -> Socket<A> {
        Self { address, handle }
    }

    pub async fn connect<'m>(
        &'m mut self,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Result<(), TcpError> {
        let m = A::connect(self.handle, proto, dst);
        A::into_response(self.address.request(m).unwrap().await)
            .unwrap()
            .connect()
    }

    pub async fn write<'m>(&'m mut self, buf: &'m [u8]) -> Result<usize, TcpError> {
        let m = A::write(self.handle, buf);
        A::into_response(self.address.request(m).unwrap().await)
            .unwrap()
            .write()
    }

    pub async fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Result<usize, TcpError> {
        let m = A::read(self.handle, buf);
        A::into_response(self.address.request(m).unwrap().await)
            .unwrap()
            .read()
    }

    pub async fn close<'m>(self) -> Result<(), TcpError> {
        let m = A::close(self.handle);
        A::into_response(self.address.request(m).unwrap().await)
            .unwrap()
            .close()
    }
}

#[cfg(feature = "tls")]
mod tls {
    use super::Socket;
    use crate::actors::tcp::TcpActor;
    use core::future::Future;
    use drogue_tls::{
        traits::{AsyncRead, AsyncWrite},
        TlsError,
    };

    impl<A> AsyncRead for Socket<A>
    where
        A: TcpActor + 'static,
    {
        type ReadFuture<'m> = impl Future<Output = Result<usize, TlsError>> + 'm
        where
            Self: 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move {
                Ok(Socket::read(self, buf)
                    .await
                    .map_err(|_| TlsError::IoError)?)
            }
        }
    }

    impl<A> AsyncWrite for Socket<A>
    where
        A: TcpActor + 'static,
    {
        type WriteFuture<'m> = impl Future<Output = Result<usize, TlsError>> + 'm
        where
            Self: 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move {
                Ok(Socket::write(self, buf)
                    .await
                    .map_err(|_| TlsError::IoError)?)
            }
        }
    }
}

#[cfg(feature = "tls")]
pub use tls::*;
