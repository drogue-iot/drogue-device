use crate::traits::{
    ip::{IpProtocol, SocketAddress},
    tcp::{TcpError, TcpStack},
};

/// A Socket type for connecting to a network endpoint + sending and receiving data.
#[derive(Clone, Copy)]
pub struct Socket<A>
where
    A: TcpStack + Clone + 'static,
{
    network: A,
    handle: A::SocketHandle,
}

impl<A> Socket<A>
where
    A: TcpStack + Clone + 'static,
{
    pub fn new(network: A, handle: A::SocketHandle) -> Socket<A> {
        Self { network, handle }
    }

    pub async fn connect<'m>(
        &'m mut self,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Result<(), TcpError> {
        self.network.connect(self.handle, proto, dst).await
    }

    pub async fn write<'m>(&'m mut self, buf: &'m [u8]) -> Result<usize, TcpError> {
        self.network.write(self.handle, buf).await
    }

    pub async fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Result<usize, TcpError> {
        self.network.read(self.handle, buf).await
    }

    pub async fn close<'m>(mut self) -> Result<(), TcpError> {
        self.network.close(self.handle).await
    }
}

#[cfg(feature = "tls")]
mod tls {
    use super::Socket;
    use crate::traits::tcp::TcpStack;
    use core::future::Future;
    use drogue_tls::{
        traits::{AsyncRead, AsyncWrite},
        TlsError,
    };

    impl<A> AsyncRead for Socket<A>
    where
        A: TcpStack + Clone + 'static,
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
        A: TcpStack + Clone + 'static,
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
