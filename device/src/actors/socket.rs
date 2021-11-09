use super::tcp::{TcpActor, TcpResponse};
use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::TcpError,
    },
};

/// A Socket type for connecting to a network endpoint + sending and receiving data.
#[derive(Clone, Copy)]
pub struct Socket<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    address: Address<'a, A>,
    handle: A::SocketHandle,
}

impl<'a, A> Socket<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    pub fn new(address: Address<'a, A>, handle: A::SocketHandle) -> Socket<'a, A> {
        Self { address, handle }
    }

    pub async fn connect<'m>(
        &'m mut self,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Result<(), TcpError> {
        let m = A::connect(self.handle, proto, dst);
        self.address.request(m).unwrap().await.into().connect()
    }

    pub async fn write<'m>(&'m mut self, buf: &'m [u8]) -> Result<usize, TcpError> {
        let m = A::write(self.handle, buf);
        self.address.request(m).unwrap().await.into().write()
    }

    pub async fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Result<usize, TcpError> {
        let m = A::read(self.handle, buf);
        self.address.request(m).unwrap().await.into().read()
    }

    pub async fn close<'m>(self) -> Result<(), TcpError> {
        let m = A::close(self.handle);
        self.address.request(m).unwrap().await.into().close()
    }
}

#[cfg(feature = "tls")]
mod tls {
    use super::Socket;
    use crate::actors::tcp::{TcpActor, TcpResponse};
    use crate::kernel::actor::Actor;
    use core::future::Future;
    use drogue_tls::{
        traits::{AsyncRead, AsyncWrite},
        TlsError,
    };

    impl<'a, A> AsyncRead for Socket<'a, A>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
    {
        type ReadFuture<'m>
        where
            Self: 'm,
        = impl Future<Output = Result<usize, TlsError>> + 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move {
                Ok(Socket::read(self, buf)
                    .await
                    .map_err(|_| TlsError::IoError)?)
            }
        }
    }

    impl<'a, A> AsyncWrite for Socket<'a, A>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
    {
        type WriteFuture<'m>
        where
            Self: 'm,
        = impl Future<Output = Result<usize, TlsError>> + 'm;
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
