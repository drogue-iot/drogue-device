use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::api::ip::{IpProtocol, SocketAddress};
use crate::driver::tls::config::Config;
use crate::driver::tls::handshake::client_hello::ClientHello;
use crate::driver::tls::handshake::Handshake;
use crate::driver::tls::record::Record;
use crate::driver::tls::TlsError;
use crate::prelude::*;
use rand_core::{CryptoRng, RngCore};

pub struct TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore,
{
    delegate: Option<Address<D>>,
    pub(crate) config: Config<RNG>,
}

impl<D, RNG> Actor for TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore,
{
    type Configuration = Address<D>;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.delegate.replace(config);
    }
}

impl<D, RNG> TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy,
{
    pub fn new(config: Config<RNG>) -> Self {
        Self {
            delegate: None,
            config,
        }
    }

    async fn handshake(&mut self, delegate: &mut TcpSocket<D>) -> Result<(), TlsError> {
        let mut client_hello = ClientHello::new(&self.config);
        client_hello.transmit(delegate).await?;

        let record = Record::parse(delegate).await?;
        match record {
            Record::Handshake(handshake) => match handshake {
                Handshake::ServerHello(server_hello) => {}
            },
            Record::Alert => {
                unimplemented!("alert not handled")
            }
            Record::ApplicationData => {
                unimplemented!("application data handled")
            }
        }

        log::info!("record -> {:?}", record);
        Ok(())
    }
}

impl<D, RNG> TcpStack for TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy,
{
    type SocketHandle = D::SocketHandle;

    fn open(self) -> Response<Self, Self::SocketHandle> {
        Response::defer(async move {
            let delegate = self.delegate.unwrap().tcp_open().await;
            let delegate = delegate.handle();
            (self, delegate)
        })
    }

    fn connect(
        mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Response<Self, Result<(), TcpError>> {
        Response::defer(async move {
            let mut delegate = TcpSocket::new(self.delegate.unwrap(), handle);
            let result = delegate.connect(proto, dst).await;
            let result = self.handshake(&mut delegate).await;
            let result = match result {
                Ok(_) => Ok(()),
                Err(TlsError::TcpError(tcp_error)) => Err(tcp_error),
                _ => Err(TcpError::ConnectError),
            };
            (self, result)
        })
    }

    fn write(
        self,
        handle: Self::SocketHandle,
        buf: &[u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        unimplemented!()
    }

    fn read(
        self,
        handle: Self::SocketHandle,
        buf: &mut [u8],
    ) -> Response<Self, Result<usize, TcpError>> {
        unimplemented!()
    }

    fn close(self, handle: Self::SocketHandle) -> Completion<Self> {
        unimplemented!()
    }
}
