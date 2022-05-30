use core::future::Future;
use embedded_nal_async::*;
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct StdTcpClient;
pub struct StdTcpConn(TcpStream);

impl embedded_io::Io for StdTcpClient {
    type Error = std::io::Error;
}

impl TcpClient for StdTcpClient {
    type TcpConnection<'m> = StdTcpConn;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::TcpConnection<'m>, Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(&'m mut self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
                Ok(stream) => Ok(StdTcpConn(stream)),
                Err(e) => Err(e),
            }
        }
    }
}

impl embedded_io::Io for StdTcpConn {
    type Error = std::io::Error;
}

impl embedded_io::asynch::Read for StdTcpConn {
    type ReadFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move { self.0.read(buf) }
    }
}

impl embedded_io::asynch::Write for StdTcpConn {
    type WriteFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { self.0.write(buf) }
    }

    type FlushFuture<'m> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move { Ok(()) }
    }
}
