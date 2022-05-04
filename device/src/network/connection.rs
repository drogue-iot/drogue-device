use super::socket::*;
use core::future::Future;
use embedded_nal_async::*;

use crate::network::tcp::TcpError;

/// Trait for network connections
pub trait ConnectionFactory: Sized {
    type Connection: NetworkConnection;
    type ConnectFuture<'m>: Future<Output = Result<Self::Connection, NetworkError>>
    where
        Self: 'm;
    fn connect<'m>(&'m mut self, host: &'m str, remote: SocketAddr) -> Self::ConnectFuture<'m>;
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NetworkError {
    Tcp(TcpError),
    #[cfg(feature = "tls")]
    Tls(embedded_tls::TlsError),
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

    type CloseFuture<'m>: Future<Output = Result<(), NetworkError>>;
    fn close<'m>(self) -> Self::CloseFuture<'m>;
}

impl<A> ConnectionFactory for A
where
    A: TcpClientStack + Clone + 'static,
{
    type Connection = Socket<A>;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection, NetworkError>> + 'm where A: 'm;

    fn connect<'m>(&'m mut self, _: &'m str, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            // info!("Allocate TLS buffer");
            let mut socket = Socket::new(
                self.clone(),
                self.socket()
                    .await
                    .map_err(|_| NetworkError::Tcp(TcpError::OpenError))?,
            );
            match socket.connect(remote).await {
                Ok(_) => {
                    trace!("Connection established");
                    Ok(socket)
                }
                Err(e) => {
                    warn!("Error creating connection: {:?}", e);
                    socket.close().await.map_err(|e| NetworkError::Tcp(e))?;
                    Err(NetworkError::Tcp(e))
                }
            }
        }
    }
}

impl<A> NetworkConnection for Socket<A>
where
    A: TcpClientStack + Clone + 'static,
{
    type WriteFuture<'m> = impl Future<Output = Result<usize, NetworkError>> + 'm where A: 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            Socket::write(self, buf)
                .await
                .map_err(|e| NetworkError::Tcp(e))
        }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, NetworkError>> + 'm where A: 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            Socket::read(self, buf)
                .await
                .map_err(|e| NetworkError::Tcp(e))
        }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), NetworkError>> + 'm where A: 'm;
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
    use crate::network::socket::*;
    use crate::network::tcp::TcpError;
    use core::cell::UnsafeCell;
    use core::future::Future;
    use core::mem::MaybeUninit;
    use embedded_nal_async::{SocketAddr, TcpClientStack};
    use embedded_tls::{NoClock, TlsCipherSuite, TlsConfig, TlsConnection, TlsContext, TlsError};
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
            //info!("Freeing TLS buffer");
            self.free.store(true, Ordering::SeqCst);
        }
    }

    pub struct TlsConnectionFactory<'a, A, CipherSuite, RNG, const N: usize>
    where
        A: TcpClientStack + Clone + 'static,
        RNG: CryptoRng + RngCore + 'a,
        CipherSuite: TlsCipherSuite + 'static,
    {
        rng: RNG,
        pool: [MaybeUninit<TlsBuffer<'a>>; N],
        network: A,
        _cipher: PhantomData<&'a CipherSuite>,
    }

    impl<'a, A, CipherSuite, RNG, const N: usize> TlsConnectionFactory<'a, A, CipherSuite, RNG, N>
    where
        A: TcpClientStack + Clone + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        pub fn new<const TLS_BUFFER_SIZE: usize>(
            network: A,
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
        A: TcpClientStack + Clone + 'static,
        RNG: CryptoRng + RngCore + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        type Connection = TlsNetworkConnection<'a, A, CipherSuite>;
        type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection, NetworkError>> + 'm
        where
            'a: 'm,
            A: 'm,
            RNG: 'm,
            CipherSuite: 'm;

        fn connect<'m>(&'m mut self, host: &'m str, remote: SocketAddr) -> Self::ConnectFuture<'m> {
            async move {
                let mut idx = 0;
                let mut buffer = None;
                for i in 0..self.pool.len() {
                    if let Some(buf) = unsafe { self.pool[i].assume_init_ref() }.allocate() {
                        idx = i;
                        buffer.replace(buf);
                        break;
                    }
                }
                if buffer.is_none() {
                    return Err(NetworkError::Tls(TlsError::OutOfMemory));
                }
                let buffer = buffer.unwrap();
                let buffer_ptr = self.pool[idx].as_ptr();

                let mut socket = Socket::new(
                    self.network.clone(),
                    self.network
                        .socket()
                        .await
                        .map_err(|_| NetworkError::Tcp(TcpError::OpenError))?,
                );
                match socket.connect(remote).await {
                    Ok(_) => {
                        trace!("Connection established");
                        let config = TlsConfig::new().with_server_name("localhost");
                        let mut tls: TlsConnection<'a, Socket<A>, CipherSuite> =
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
        A: TcpClientStack + Clone + 'static,
        CipherSuite: TlsCipherSuite + 'static,
    {
        buffer: *const TlsBuffer<'a>,
        connection: TlsConnection<'a, Socket<A>, CipherSuite>,
    }

    impl<'a, A, CipherSuite> TlsNetworkConnection<'a, A, CipherSuite>
    where
        A: TcpClientStack + Clone + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        pub fn new(
            connection: TlsConnection<'a, Socket<A>, CipherSuite>,
            buffer: *const TlsBuffer<'a>,
        ) -> Self {
            Self { connection, buffer }
        }

        pub async fn write(&mut self, buf: &[u8]) -> Result<usize, NetworkError> {
            self.connection
                .write(buf)
                .await
                .map_err(|e| NetworkError::Tls(e))
        }

        pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetworkError> {
            self.connection
                .read(buf)
                .await
                .map_err(|e| NetworkError::Tls(e))
        }

        pub async fn close(self) -> Result<(), NetworkError> {
            let result = match self.connection.close().await {
                Ok(socket) => NetworkConnection::close(socket).await,
                Err(e) => Err(NetworkError::Tls(e)),
            };
            unsafe { &*self.buffer }.free();
            result
        }
    }

    impl<'a, A, CipherSuite> NetworkConnection for TlsNetworkConnection<'a, A, CipherSuite>
    where
        A: TcpClientStack + Clone + 'static,
        CipherSuite: TlsCipherSuite + 'a,
    {
        type WriteFuture<'m> = impl Future<Output = Result<usize, NetworkError>> + 'm
        where
            Self: 'm;
        fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
            async move { TlsNetworkConnection::write(self, buf).await }
        }

        type ReadFuture<'m> = impl Future<Output = Result<usize, NetworkError>> + 'm
        where
            Self: 'm;
        fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
            async move { TlsNetworkConnection::read(self, buf).await }
        }

        type CloseFuture<'m> = impl Future<Output = Result<(), NetworkError>>;
        fn close<'m>(self) -> Self::CloseFuture<'m> {
            async move { TlsNetworkConnection::close(self).await }
        }
    }
}
