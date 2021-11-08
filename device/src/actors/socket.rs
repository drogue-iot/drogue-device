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

    #[rustfmt::skip]
    type OpenFuture<'m> where A: 'm =  impl Future<Output = Result<Self::Socket, TcpError>> + 'm;
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
        tcp::{SocketFactory, TcpError, TcpSocket},
    };
    use core::cell::UnsafeCell;
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

        pub async fn free<'m>(&'m mut self) -> Option<TlsContext<'a, CipherSuite, RNG, NoClock>> {
            match self.state.take() {
                Some(State::Connected(session)) => match session.close().await {
                    Ok((context, mut socket)) => {
                        socket.close().await;
                        Some(context)
                    }
                    _ => None,
                },
                Some(State::New(context, mut socket)) => {
                    socket.close().await;
                    Some(context)
                }
                None => None,
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

    pub struct TlsSingleSocketFactory<'a, S, RNG, CipherSuite>
    where
        S: SocketFactory + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
        S::Socket: AsyncWrite + AsyncRead + 'static,
    {
        factory: S,
        holder: UnsafeCell<SocketHolder<'a, S::Socket, RNG, CipherSuite>>,
    }

    impl<'a, S, RNG, CipherSuite> TlsSingleSocketFactory<'a, S, RNG, CipherSuite>
    where
        S: SocketFactory + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
        S::Socket: AsyncWrite + AsyncRead + 'a,
    {
        pub fn new(factory: S, context: TlsContext<'a, CipherSuite, RNG, NoClock>) -> Self {
            Self {
                factory,
                holder: UnsafeCell::new(SocketHolder {
                    context: Some(context),
                    socket: None,
                }),
            }
        }
    }

    impl<'a, S, RNG, CipherSuite> SocketFactory for TlsSingleSocketFactory<'a, S, RNG, CipherSuite>
    where
        S: SocketFactory + 'a,
        RNG: CryptoRng + RngCore + 'a,
        CipherSuite: TlsCipherSuite + 'a,
        S::Socket: AsyncWrite + AsyncRead + 'a,
    {
        type Socket = ReusableSocket<'a, S::Socket, RNG, CipherSuite>;

        #[rustfmt::skip]
        type OpenFuture<'m> where 'a: 'm, S: 'm, RNG: 'm, CipherSuite: 'm =  impl Future<Output = Result<Self::Socket, TcpError>> + 'm;
        fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
            async move {
                match self.factory.open().await {
                    Ok(h) => {
                        let holder = unsafe { &mut *self.holder.get() };
                        holder.open(h)?;
                        Ok(ReusableSocket { holder })
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub struct SocketHolder<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        socket: Option<TlsSocket<'a, S, RNG, CipherSuite>>,
        context: Option<TlsContext<'a, CipherSuite, RNG, NoClock>>,
    }

    impl<'a, S, RNG, CipherSuite> SocketHolder<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        fn open(&mut self, socket: S) -> Result<(), TcpError> {
            match self.context.take() {
                Some(context) => {
                    self.socket.replace(TlsSocket::wrap(socket, context));
                    Ok(())
                }
                _ => Err(TcpError::OpenError),
            }
        }

        fn socket(&mut self) -> &mut TlsSocket<'a, S, RNG, CipherSuite> {
            match self.socket.as_mut() {
                Some(socket) => socket,
                _ => {
                    panic!("referencing unopened socket!");
                }
            }
        }

        async fn close(&mut self) {
            match self.socket.take() {
                Some(mut socket) => {
                    if let Some(context) = socket.free().await {
                        self.context.replace(context);
                    }
                }
                _ => {
                    panic!("attempting to unopened socket")
                }
            }
        }
    }

    pub struct ReusableSocket<'a, S, RNG, CipherSuite>
    where
        S: TcpSocket + AsyncWrite + AsyncRead + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        holder: &'a mut SocketHolder<'a, S, RNG, CipherSuite>,
    }

    impl<'a, S, RNG, CipherSuite> TcpSocket for ReusableSocket<'a, S, RNG, CipherSuite>
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
            self.holder.socket().connect(proto, dst)
        }

            #[rustfmt::skip]
        type WriteFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            self.holder.socket().write(buf)
        }

            #[rustfmt::skip]
        type ReadFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            self.holder.socket().read(buf)
        }

            #[rustfmt::skip]
        type CloseFuture<'m> where 'a: 'm, RNG: 'm, CipherSuite: 'm = impl Future<Output = ()> + 'm;
        fn close<'m>(&'m mut self) -> Self::CloseFuture<'m> {
            self.holder.close()
        }
    }
}

#[cfg(feature = "tls")]
pub use tls::*;
