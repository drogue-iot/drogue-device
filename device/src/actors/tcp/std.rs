use super::{TcpActor, TcpRequest, TcpResponse};
use crate::drivers::common::socket_pool::SocketPool;
use crate::kernel::actor::{Actor, Address, Inbox};
use crate::traits::ip::{IpProtocol, SocketAddress};
use crate::traits::tcp::TcpError;
use core::future::Future;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct StdTcpActor {
    sockets: HashMap<u8, TcpStream>,
    socket_pool: SocketPool,
}

impl StdTcpActor {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            socket_pool: SocketPool::new(),
        }
    }
}

impl Actor for StdTcpActor {
    type Configuration = ();
    type Message<'m> = TcpRequest<'m, u8>;
    type Response = Option<TcpResponse<u8>>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                let mut m = inbox.next().await;
                let response = match m.message() {
                    TcpRequest::Open => {
                        let handle = self
                            .socket_pool
                            .open()
                            .await
                            .map_err(|_| TcpError::OpenError);
                        TcpResponse::Open(handle)
                    }
                    TcpRequest::Connect(handle, _proto, addr) => {
                        match TcpStream::connect(format!("{}:{}", addr.ip(), addr.port())) {
                            Ok(stream) => {
                                self.sockets.insert(*handle, stream);
                                TcpResponse::Connect(Ok(()))
                            }
                            _ => TcpResponse::Connect(Err(TcpError::ConnectError)),
                        }
                    }
                    TcpRequest::Write(handle, buf) => {
                        if let Some(mut s) = self.sockets.get(handle) {
                            match s.write(buf) {
                                Ok(sz) => TcpResponse::Write(Ok(sz)),
                                _ => TcpResponse::Write(Err(TcpError::WriteError)),
                            }
                        } else {
                            TcpResponse::Write(Err(TcpError::WriteError))
                        }
                    }
                    TcpRequest::Read(handle, buf) => {
                        if let Some(mut s) = self.sockets.get(handle) {
                            match s.read(buf) {
                                Ok(sz) => TcpResponse::Read(Ok(sz)),
                                _ => TcpResponse::Read(Err(TcpError::ReadError)),
                            }
                        } else {
                            TcpResponse::Read(Err(TcpError::ReadError))
                        }
                    }
                    TcpRequest::Close(handle) => {
                        if self.sockets.remove(handle).is_some() {
                            // Move through both close states
                            self.socket_pool.close(*handle);
                            self.socket_pool.close(*handle);
                        }
                        TcpResponse::Close(Ok(()))
                    }
                };
                m.set_response(Some(response));
            }
        }
    }
}

impl TcpActor<StdTcpActor> for StdTcpActor {
    type SocketHandle = u8;

    fn open<'m>() -> TcpRequest<'m, u8> {
        TcpRequest::Open
    }
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> TcpRequest<'m, u8> {
        TcpRequest::Connect(handle, proto, dst)
    }
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> TcpRequest<'m, u8> {
        TcpRequest::Write(handle, buf)
    }
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> TcpRequest<'m, u8> {
        TcpRequest::Read(handle, buf)
    }
    fn close<'m>(handle: Self::SocketHandle) -> TcpRequest<'m, u8> {
        TcpRequest::Close(handle)
    }
}

impl Into<TcpResponse<u8>> for Option<TcpResponse<u8>> {
    fn into(self) -> TcpResponse<u8> {
        match self {
            Some(r) => r,
            _ => panic!("cannot convert response to tcp response"),
        }
    }
}
