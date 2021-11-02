use super::tcp::{TcpActor, TcpResponse};
use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::{TcpError, TcpSocket},
    },
};
use core::future::Future;

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
}

impl<'a, A> TcpSocket for Socket<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    #[rustfmt::skip]
    type ConnectFuture<'m> where 'a: 'm, A: 'm =  impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(&'m mut self, proto: IpProtocol, dst: SocketAddress) -> Self::ConnectFuture<'m> {
        async move {
            let m = A::connect(self.handle, proto, dst);
            self.address.request(m).unwrap().await.into().connect()
        }
    }

    #[rustfmt::skip]
    type WriteFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            let m = A::write(self.handle, buf);
            self.address.request(m).unwrap().await.into().write()
        }
    }

    #[rustfmt::skip]
    type ReadFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            let m = A::read(self.handle, buf);
            self.address.request(m).unwrap().await.into().read()
        }
    }

    #[rustfmt::skip]
    type CloseFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = ()> + 'm;
    fn close<'m>(&'m mut self) -> Self::CloseFuture<'m> {
        async move {
            let m = A::close(self.handle);
            self.address.request(m).unwrap().await;
        }
    }
}

#[cfg(feature = "tls")]
mod tls {
    use super::Socket;
    use crate::actors::tcp::{TcpActor, TcpResponse};
    use crate::kernel::actor::Actor;
    use crate::traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::{TcpError, TcpSocket},
    };
    use core::future::Future;
    use drogue_tls::{
        traits::{AsyncRead, AsyncWrite},
        NoClock, TlsCipherSuite, TlsConnection, TlsContext, TlsError,
    };
    use rand_core::{CryptoRng, RngCore};

    impl<'a, A> AsyncRead for Socket<'a, A>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
    {
        #[rustfmt::skip]
        type ReadFuture<'m> where Self: 'm = impl Future<Output = Result<usize, TlsError>> + 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move {
                Ok(TcpSocket::read(self, buf)
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
        #[rustfmt::skip]
        type WriteFuture<'m> where Self: 'm = impl Future<Output = Result<usize, TlsError>> + 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move {
                Ok(TcpSocket::write(self, buf)
                    .await
                    .map_err(|_| TlsError::IoError)?)
            }
        }
    }

    enum State<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        New(TlsContext<'a, CipherSuite, RNG, NoClock>, S),
        Connected(TlsConnection<'a, RNG, NoClock, S, CipherSuite>),
    }

    pub struct TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        state: Option<State<'a, S, RNG, CipherSuite>>,
    }

    impl<'a, S, RNG, CipherSuite> TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        pub fn wrap(socket: S, context: TlsContext<'a, CipherSuite, RNG, NoClock>) -> Self {
            Self {
                state: Some(State::New(context, socket)),
            }
        }
    }

    impl<'a, S, RNG, CipherSuite> TcpSocket for TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        #[rustfmt::skip]
        type ConnectFuture<'m> where 'a: 'm, S: 'm, RNG: 'm, CipherSuite: 'm =  impl Future<Output = Result<(), TcpError>> + 'm;
        fn connect<'m>(
            &'m mut self,
            proto: IpProtocol,
            dst: SocketAddress,
        ) -> Self::ConnectFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::New(context, mut socket)) => {
                        match socket.connect(proto, dst).await {
                            Ok(_) => {
                                let mut tls: TlsConnection<'a, RNG, NoClock, S, CipherSuite> =
                                    TlsConnection::new(context, socket);
                                // FIXME: support configuring cert size when verification is supported on ARM Cortex M
                                match tls.open::<1>().await {
                                    Ok(_) => {
                                        self.state.replace(State::Connected(tls));
                                        Ok(())
                                    }
                                    Err(e) => {
                                        info!("TLS connection failed: {:?}", e);
                                        match tls.close().await {
                                            Ok((context, socket)) => {
                                                self.state.replace(State::New(context, socket));
                                                Err(TcpError::ConnectError)
                                            }
                                            Err(e) => {
                                                info!("Error closing TLS connection: {:?}", e);
                                                Err(TcpError::ConnectError)
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("TCP connection failed: {:?}", e);
                                self.state.replace(State::New(context, socket));
                                Err(e)
                            }
                        }
                    }
                    Some(other) => {
                        self.state.replace(other);
                        Err(TcpError::ConnectError)
                    }
                    None => Err(TcpError::SocketClosed),
                }
            }
        }

        #[rustfmt::skip]
        type WriteFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::Connected(mut session)) => {
                        let result = session.write(buf).await.map_err(|_| TcpError::WriteError);
                        self.state.replace(State::Connected(session));
                        result
                    }
                    Some(other) => {
                        self.state.replace(other);
                        Err(TcpError::SocketClosed)
                    }
                    None => Err(TcpError::SocketClosed),
                }
            }
        }

        #[rustfmt::skip]
        type ReadFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::Connected(mut session)) => {
                        let result = session.read(buf).await.map_err(|_| TcpError::ReadError);
                        self.state.replace(State::Connected(session));
                        result
                    }
                    Some(other) => {
                        self.state.replace(other);
                        Err(TcpError::SocketClosed)
                    }
                    None => Err(TcpError::SocketClosed),
                }
            }
        }

        #[rustfmt::skip]
        type CloseFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = ()> + 'm;
        fn close<'m>(&'m mut self) -> Self::CloseFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::Connected(session)) => match session.close().await {
                        Ok((_, mut socket)) => {
                            socket.close().await;
                        }
                        _ => {}
                    },
                    Some(State::New(_, mut socket)) => {
                        socket.close().await;
                    }
                    None => {}
                }
            }
        }
    }
}

#[cfg(feature = "tls")]
pub use tls::*;
