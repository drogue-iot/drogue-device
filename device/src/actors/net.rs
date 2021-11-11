use super::socket::*;
use super::tcp::*;
use crate::traits::{ip::*, tcp::*};
use crate::{Actor, Address};
use core::future::Future;

/// Trait for network connections
pub trait ConnectionFactory: Sized {
    type Connection: NetworkConnection;
    type ConnectFuture<'m>: Future<Output = Result<Self::Connection, NetworkError>>
    where
        Self: 'm;
    fn connect<'m>(
        &'m mut self,
        host: &'m str,
        ip: IpAddress,
        port: u16,
    ) -> Self::ConnectFuture<'m>;
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NetworkError {
    Tcp(TcpError),
    #[cfg(feature = "tls")]
    Tls(drogue_tls::TlsError),
}

pub trait NetworkConnection {
    type WriteFuture<'m>: Future<Output = Result<usize, NetworkError>>
    where
        Self: 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m>;

    type ReadFuture<'m>: Future<Output = Result<usize, NetworkError>>
    where
        Self: 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m>;

    type CloseFuture<'m>: Future<Output = Result<(), NetworkError>>
    where
        Self: 'm;
    fn close<'m>(self) -> Self::CloseFuture<'m>;
}

impl<'a, A> ConnectionFactory for Address<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    type Connection = Socket<'a, A>;
    type ConnectFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<Self::Connection, NetworkError>> + 'm;

    fn connect<'m>(&'m mut self, _: &'m str, ip: IpAddress, port: u16) -> Self::ConnectFuture<'m> {
        async move {
            info!("Allocate TLS buffer");
            let mut socket = Socket::new(self.clone(), self.open().await.unwrap());
            match socket
                .connect(IpProtocol::Tcp, SocketAddress::new(ip, port))
                .await
            {
                Ok(_) => {
                    trace!("Connection established");
                    Ok(socket)
                }
                Err(e) => {
                    warn!("Error creating connection: {:?}", e);
                    Err(NetworkError::Tcp(e))
                }
            }
        }
    }
}

impl<'a, A> NetworkConnection for Socket<'a, A>
where
    A: Actor + TcpActor<A> + 'static,
    A::Response: Into<TcpResponse<A::SocketHandle>>,
{
    type WriteFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<usize, NetworkError>> + 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            Socket::write(self, buf)
                .await
                .map_err(|e| NetworkError::Tcp(e))
        }
    }

    type ReadFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<usize, NetworkError>> + 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            Socket::read(self, buf)
                .await
                .map_err(|e| NetworkError::Tcp(e))
        }
    }

    type CloseFuture<'m>
    where
        'a: 'm,
        A: 'm,
    = impl Future<Output = Result<(), NetworkError>> + 'm;
    fn close<'m>(self) -> Self::CloseFuture<'m> {
        async move { Socket::close(self).await.map_err(|e| NetworkError::Tcp(e)) }
    }
}

#[cfg(feature = "tls")]
pub use tls::*;

#[cfg(feature = "tls")]
mod tls {
    use super::NetworkConnection;
    use super::NetworkError;
    use crate::actors::{
        socket::*,
        tcp::{TcpActor, TcpResponse},
    };
    use crate::kernel::actor::Actor;
    use crate::traits::ip::{IpAddress, IpProtocol, SocketAddress};
    use crate::Address;
    use core::cell::UnsafeCell;
    use core::future::Future;
    use core::mem::MaybeUninit;
    use drogue_tls::{NoClock, TlsCipherSuite, TlsConfig, TlsConnection, TlsContext, TlsError};
    use rand_core::{CryptoRng, RngCore};

    use atomic_polyfill::{AtomicBool, Ordering};
    use core::marker::PhantomData;

    pub struct TlsBuffer<'a> {
        buf: UnsafeCell<&'a mut [u8]>,
        free: AtomicBool,
    }

    impl<'a> TlsBuffer<'a> {
        pub fn new(buf: &'a mut [u8]) -> Self {
            Self {
                buf: UnsafeCell::new(buf),
                free: AtomicBool::new(true),
            }
        }
    }

    impl<'a> TlsBuffer<'a> {
        pub fn allocate(&self) -> Option<&'a mut [u8]> {
            if self.free.swap(false, Ordering::SeqCst) {
                Some(unsafe { &mut *self.buf.get() })
            } else {
                None
            }
        }

        pub fn free(&self) {
            info!("Freeing TLS buffer");
            self.free.store(true, Ordering::SeqCst);
        }
    }

    pub struct TlsConnectionFactory<'a, A, CipherSuite, RNG, const N: usize>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        RNG: CryptoRng + RngCore + 'a,
        CipherSuite: TlsCipherSuite + 'static,
    {
        rng: RNG,
        pool: [MaybeUninit<TlsBuffer<'a>>; N],
        network: Address<'a, A>,
        _cipher: PhantomData<&'a CipherSuite>,
    }

    impl<'a, A, CipherSuite, RNG, const N: usize> TlsConnectionFactory<'a, A, CipherSuite, RNG, N>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        pub fn new<const TLS_BUFFER_SIZE: usize>(
            network: Address<'a, A>,
            rng: RNG,
            buffers: [&'a mut [u8; TLS_BUFFER_SIZE]; N],
        ) -> Self {
            let mut pool: [MaybeUninit<TlsBuffer<'_>>; N] =
                unsafe { MaybeUninit::uninit().assume_init() };

            let mut i = 0;
            for buf in buffers {
                pool[i].write(TlsBuffer::new(buf));
                i += 1;
            }
            Self {
                network,
                rng,
                pool,
                _cipher: PhantomData,
            }
        }
    }

    impl<'a, A, CipherSuite, RNG, const N: usize> super::ConnectionFactory
        for TlsConnectionFactory<'a, A, CipherSuite, RNG, N>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        type Connection = TlsNetworkConnection<'a, A, CipherSuite>;
        type ConnectFuture<'m>
        where
            'a: 'm,
            A: 'm,
            RNG: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<Self::Connection, NetworkError>> + 'm;

        fn connect<'m>(
            &'m mut self,
            host: &'m str,
            ip: IpAddress,
            port: u16,
        ) -> Self::ConnectFuture<'m> {
            async move {
                let mut idx = 0;
                let mut buffer = None;
                for i in 0..self.pool.len() {
                    if let Some(buf) = unsafe { self.pool[i].assume_init_ref() }.allocate() {
                        idx = i;
                        buffer.replace(buf);
                    }
                }
                if buffer.is_none() {
                    return Err(NetworkError::Tls(TlsError::OutOfMemory));
                }
                let buffer = buffer.unwrap();
                let buffer_ptr = self.pool[idx].as_ptr();

                let mut socket =
                    Socket::new(self.network.clone(), self.network.open().await.unwrap());
                match socket
                    .connect(IpProtocol::Tcp, SocketAddress::new(ip, port))
                    .await
                {
                    Ok(_) => {
                        trace!("Connection established");
                        let config = TlsConfig::new().with_server_name(host);
                        let mut tls: TlsConnection<'a, Socket<'a, A>, CipherSuite> =
                            TlsConnection::new(socket, buffer);
                        // FIXME: support configuring cert size when verification is supported on ARM Cortex M
                        match tls
                            .open::<RNG, NoClock, 1>(TlsContext::new(&config, &mut self.rng))
                            .await
                        {
                            Ok(_) => Ok(TlsNetworkConnection::new(tls, buffer_ptr)),
                            Err(e) => {
                                warn!("Error creating TLS session: {:?}", e);
                                unsafe { &*buffer_ptr }.free();
                                Err(NetworkError::Tls(e))
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error creating connection: {:?}", e);
                        unsafe { &*buffer_ptr }.free();
                        Err(NetworkError::Tcp(e))
                    }
                }
            }
        }
    }

    pub struct TlsNetworkConnection<'a, A, CipherSuite>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        CipherSuite: TlsCipherSuite + 'static,
    {
        buffer: *const TlsBuffer<'a>,
        connection: TlsConnection<'a, Socket<'a, A>, CipherSuite>,
    }

    impl<'a, A, CipherSuite> TlsNetworkConnection<'a, A, CipherSuite>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        CipherSuite: TlsCipherSuite + 'a,
    {
        pub fn new(
            connection: TlsConnection<'a, Socket<'a, A>, CipherSuite>,
            buffer: *const TlsBuffer<'a>,
        ) -> Self {
            Self { connection, buffer }
        }
    }

    impl<'a, A, CipherSuite> NetworkConnection for TlsNetworkConnection<'a, A, CipherSuite>
    where
        A: Actor + TcpActor<A> + 'static,
        A::Response: Into<TcpResponse<A::SocketHandle>>,
        CipherSuite: TlsCipherSuite + 'a,
    {
        type WriteFuture<'m>
        where
            'a: 'm,
            A: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<usize, NetworkError>> + 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move {
                self.connection
                    .write(buf)
                    .await
                    .map_err(|e| NetworkError::Tls(e))
            }
        }

        type ReadFuture<'m>
        where
            'a: 'm,
            A: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<usize, NetworkError>> + 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move {
                self.connection
                    .read(buf)
                    .await
                    .map_err(|e| NetworkError::Tls(e))
            }
        }

        type CloseFuture<'m>
        where
            'a: 'm,
            A: 'm,
            CipherSuite: 'm,
        = impl Future<Output = Result<(), NetworkError>> + 'm;
        fn close<'m>(self) -> Self::CloseFuture<'m> {
            async move {
                let result = match self.connection.close().await {
                    Ok(socket) => NetworkConnection::close(socket).await,
                    Err(e) => Err(NetworkError::Tls(e)),
                };
                unsafe { &*self.buffer }.free();
                result
            }
        }
    }
}
