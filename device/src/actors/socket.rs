use super::tcp::{TcpActor, TcpResponse};
use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpProtocol, SocketAddress},
        tcp::{SocketFactory, TcpError, TcpSocket},
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

impl<A> SocketFactory for Address<'static, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    type Socket = Socket<'static, A>;

    type OpenFuture<'m>
    where
        A: 'm,
    = impl Future<Output = Result<Self::Socket, TcpError>> + 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            let m = A::open();
            match self.request(m).unwrap().await.into().open() {
                Ok(h) => Ok(Socket::new(self.clone(), h)),
                Err(e) => Err(e),
            }
        }
    }
}

impl<'a, A> TcpSocket for Socket<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    type ConnectFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(&'m mut self, proto: IpProtocol, dst: SocketAddress) -> Self::ConnectFuture<'m> {
        async move {
            let m = A::connect(self.handle, proto, dst);
            self.address.request(m).unwrap().await.into().connect()
        }
    }

    type WriteFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            let m = A::write(self.handle, buf);
            self.address.request(m).unwrap().await.into().write()
        }
    }

    type ReadFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            let m = A::read(self.handle, buf);
            self.address.request(m).unwrap().await.into().read()
        }
    }

    type CloseFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<(), TcpError>> + 'm;
    fn close<'m>(self) -> Self::CloseFuture<'m> {
        async move {
            let m = A::close(self.handle);
            self.address.request(m).unwrap().await.into().close()
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
        NoClock, TlsCipherSuite, TlsConfig, TlsConnection, TlsContext, TlsError,
    };
    use rand_core::{CryptoRng, RngCore};

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
        type WriteFuture<'m>
        where
            Self: 'm,
        = impl Future<Output = Result<usize, TlsError>> + 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move {
                Ok(TcpSocket::write(self, buf)
                    .await
                    .map_err(|_| TlsError::IoError)?)
            }
        }
    }

    pub struct TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        state: Option<State<'a, S, RNG, CipherSuite>>,
    }

    enum State<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        New(S, &'a mut [u8], TlsConfig<'a, CipherSuite>, RNG),
        Connected(TlsConnection<'a, S, CipherSuite>),
    }

    impl<'a, S, RNG, CipherSuite> TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        pub fn wrap(
            socket: S,
            record_buffer: &'a mut [u8],
            config: TlsConfig<'a, CipherSuite>,
            rng: RNG,
        ) -> Self {
            Self {
                state: Some(State::New(socket, record_buffer, config, rng)),
            }
        }
    }

    impl<'a, S, RNG, CipherSuite> TcpSocket for TlsSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        type ConnectFuture<'m>
        where
            'a: 'm,
            S: 'm,
            RNG: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<(), TcpError>> + 'm;
        fn connect<'m>(
            &'m mut self,
            proto: IpProtocol,
            dst: SocketAddress,
        ) -> Self::ConnectFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::New(mut socket, record_buffer, config, mut rng)) => {
                        match socket.connect(proto, dst).await {
                            Ok(_) => {
                                let mut tls: TlsConnection<'a, S, CipherSuite> =
                                    TlsConnection::new(socket, record_buffer);
                                // FIXME: support configuring cert size when verification is supported on ARM Cortex M
                                match tls
                                    .open::<RNG, NoClock, 1>(TlsContext::new(&config, &mut rng))
                                    .await
                                {
                                    Ok(_) => {
                                        self.state.replace(State::Connected(tls));
                                        Ok(())
                                    }
                                    Err(e) => {
                                        info!("TLS connection failed: {:?}", e);
                                        tls.close().await.map_err(|_| TcpError::CloseError)?;
                                        Err(TcpError::ConnectError)
                                    }
                                }
                            }
                            Err(e) => {
                                info!("TCP connection failed: {:?}", e);
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

        type WriteFuture<'m>
        where
            'a: 'm,
            RNG: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<usize, TcpError>> + 'm;
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

        type ReadFuture<'m>
        where
            'a: 'm,
            RNG: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<usize, TcpError>> + 'm;
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

        type CloseFuture<'m>
        where
            'a: 'm,
            RNG: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<(), TcpError>> + 'm;
        fn close<'m>(mut self) -> Self::CloseFuture<'m> {
            async move {
                match self.state.take() {
                    Some(State::Connected(session)) => match session.close().await {
                        Ok(socket) => socket.close().await,
                        Err(_) => Err(TcpError::CloseError),
                    },
                    Some(State::New(socket, _, _, _)) => socket.close().await,
                    None => Ok(()),
                }
            }
        }
    }
}

#[cfg(feature = "tls")]
pub use tls::*;
