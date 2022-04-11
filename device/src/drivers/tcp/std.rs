use crate::drivers::common::socket_pool::SocketPool;
use crate::traits::ip::{IpProtocol, SocketAddress};
use crate::traits::tcp::{TcpError, TcpStack};
use core::future::Future;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct StdTcpStack {
    sockets: HashMap<u8, TcpStream>,
    socket_pool: SocketPool,
}

impl StdTcpStack {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            socket_pool: SocketPool::new(),
        }
    }
}

impl Default for StdTcpStack {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpStack for StdTcpStack {
    type SocketHandle = u8;

    type OpenFuture<'m> = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm
    where
        Self: 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            self.socket_pool
                .open()
                .await
                .map_err(|_| TcpError::OpenError)
        }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        _: IpProtocol,
        addr: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", addr.ip(), addr.port())) {
                Ok(stream) => {
                    self.sockets.insert(handle, stream);
                    Ok(())
                }
                _ => Err(TcpError::ConnectError),
            }
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            if let Some(mut s) = self.sockets.get(&handle) {
                match s.write(buf) {
                    Ok(sz) => Ok(sz),
                    _ => Err(TcpError::WriteError),
                }
            } else {
                Err(TcpError::WriteError)
            }
        }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            if let Some(mut s) = self.sockets.get(&handle) {
                match s.read(buf) {
                    Ok(sz) => Ok(sz),
                    _ => Err(TcpError::ReadError),
                }
            } else {
                Err(TcpError::ReadError)
            }
        }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            if self.sockets.remove(&handle).is_some() {
                // Move through both close states
                self.socket_pool.close(handle);
                self.socket_pool.close(handle);
            }
            Ok(())
        }
    }
}
