use crate::drivers::common::socket_pool::SocketPool;
use core::future::Future;
use embedded_nal_async::*;
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

impl TcpClientStack for StdTcpStack {
    type TcpSocket = u8;
    type Error = std::io::Error;

    type SocketFuture<'m> = impl Future<Output = Result<Self::TcpSocket, Self::Error>> + 'm
    where
        Self: 'm;
    fn socket<'m>(&'m mut self) -> Self::SocketFuture<'m> {
        async move {
            self.socket_pool
                .open()
                .await
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Error opening socket"))
        }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(
        &'m mut self,
        socket: &'m mut Self::TcpSocket,
        remote: SocketAddr,
    ) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
                Ok(stream) => {
                    self.sockets.insert(*socket, stream);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    }

    type IsConnectedFuture<'m> =
        impl Future<Output = Result<bool, Self::Error>> + 'm where Self: 'm;
    fn is_connected<'m>(&'m mut self, _socket: &'m Self::TcpSocket) -> Self::IsConnectedFuture<'m> {
        async move { todo!() }
    }

    type SendFuture<'m> =
        impl Future<Output = Result<usize, Self::Error>> + 'm where Self: 'm;
    fn send<'m>(
        &'m mut self,
        handle: &'m mut Self::TcpSocket,
        buffer: &'m [u8],
    ) -> Self::SendFuture<'m> {
        async move {
            if let Some(mut s) = self.sockets.get(handle) {
                s.write(buffer)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error writing to socket",
                ))
            }
        }
    }

    type ReceiveFuture<'m> =
        impl Future<Output = Result<usize, Self::Error>> + 'm where Self: 'm;
    fn receive<'m>(
        &'m mut self,
        socket: &'m mut Self::TcpSocket,
        buffer: &'m mut [u8],
    ) -> Self::ReceiveFuture<'m> {
        async move {
            if let Some(mut s) = self.sockets.get(socket) {
                s.read(buffer)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error reading from socket",
                ))
            }
        }
    }

    type CloseFuture<'m> =
        impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn close<'m>(&'m mut self, handle: Self::TcpSocket) -> Self::CloseFuture<'m> {
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
