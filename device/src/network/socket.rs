use embedded_nal_async::*;

use crate::network::tcp::TcpError;

/// A Socket type for connecting to a network endpoint + sending and receiving data.
#[derive(Clone, Copy)]
pub struct Socket<A>
where
    A: TcpClientStack + Clone + 'static,
{
    network: A,
    handle: A::TcpSocket,
}

impl<A> Socket<A>
where
    A: TcpClientStack + Clone + 'static,
{
    pub fn new(network: A, handle: A::TcpSocket) -> Socket<A> {
        Self { network, handle }
    }

    pub async fn connect<'m>(&'m mut self, remote: SocketAddr) -> Result<(), TcpError> {
        self.network
            .connect(&mut self.handle, remote)
            .await
            .map_err(|_| TcpError::ConnectError)
    }

    pub async fn write<'m>(&'m mut self, buf: &'m [u8]) -> Result<usize, TcpError> {
        self.network
            .send(&mut self.handle, buf)
            .await
            .map_err(|_| TcpError::WriteError)
    }

    pub async fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Result<usize, TcpError> {
        self.network
            .receive(&mut self.handle, buf)
            .await
            .map_err(|_| TcpError::ReadError)
    }

    pub async fn close<'m>(mut self) -> Result<(), TcpError> {
        self.network
            .close(self.handle)
            .await
            .map_err(|_| TcpError::CloseError)
    }
}

#[cfg(feature = "tls")]
mod tls {
    use super::Socket;
    use core::future::Future;
    use drogue_tls::{
        traits::{AsyncRead, AsyncWrite},
        TlsError,
    };
    use embedded_nal_async::*;

    impl<A> AsyncRead for Socket<A>
    where
        A: TcpClientStack + Clone + 'static,
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
        A: TcpClientStack + Clone + 'static,
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
