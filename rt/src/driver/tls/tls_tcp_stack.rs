use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::api::ip::{IpProtocol, SocketAddress};
use crate::driver::tls::config::Config;
use crate::driver::tls::crypto_engine::CryptoEngine;
use crate::driver::tls::handshake::client_hello::ClientHello;
use crate::driver::tls::handshake::server_hello::ServerHello;
use crate::driver::tls::handshake::ServerHandshake;
use crate::driver::tls::record::{ClientRecord, ServerRecord};
use crate::driver::tls::tls_connection::TlsConnection;
use crate::driver::tls::TlsError;
use crate::prelude::*;
use heapless::{consts::*, Vec};
use rand_core::{CryptoRng, RngCore};

pub struct TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy + 'static,
{
    delegate: Option<Address<D>>,
    pub(crate) config: Option<&'static Config<RNG>>,
    connections: [Option<TlsConnection<RNG, D>>; 5],
}

impl<D, RNG> Actor for TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy + 'static,
{
    type Configuration = (&'static Config<RNG>, Address<D>);

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.config.replace(config.0);
        self.delegate.replace(config.1);
    }
}

impl<D, RNG> TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy,
{
    pub fn new() -> Self {
        Self {
            delegate: None,
            config: None,
            connections: Default::default(),
        }
    }
}

impl<D, RNG> TcpStack for TlsTcpStack<D, RNG>
where
    D: TcpStack + 'static,
    RNG: CryptoRng + RngCore + Copy,
{
    type SocketHandle = u8;

    fn open(mut self) -> Response<Self, Self::SocketHandle> {
        Response::defer(async move {
            let delegate = self.delegate.unwrap().tcp_open().await;
            //let handle = TlsConnection::new(self.delegate.unwrap(), delegate);
            let result = self
                .connections
                .iter_mut()
                .enumerate()
                .find(|(index, slot)| matches!(slot, None));

            match result {
                None => (self, u8::max_value()),
                Some((index, slot)) => {
                    slot.replace(TlsConnection::new(self.config.unwrap(), delegate));
                    (self, index as u8)
                }
            }
        })
    }

    fn connect(
        mut self,
        mut handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Response<Self, Result<(), TcpError>> {
        Response::defer(async move {
            let mut connection = &mut self.connections[handle as usize];

            match connection {
                None => (self, Err(TcpError::ConnectError)),
                Some(connection) => {
                    let result = connection.connect(proto, dst).await.map_err(|e| match e {
                        TlsError::TcpError(tcp_error) => tcp_error,
                        _ => TcpError::ConnectError,
                    });
                    (self, result)
                }
            }
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
