use core::future::Future;
use embedded_nal_async::*;
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct StdTcpClientSocket {
    socket: Option<TcpStream>,
}

impl Default for StdTcpClientSocket {
    fn default() -> Self {
        Self { socket: None }
    }
}

impl embedded_io::Io for StdTcpClientSocket {
    type Error = std::io::Error;
}

impl TcpClientSocket for StdTcpClientSocket {
    type ConnectFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(&'m mut self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
                Ok(stream) => {
                    self.socket.replace(stream);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    }

    type IsConnectedFuture<'m> = impl Future<Output = Result<bool, Self::Error>> + 'm
    where
        Self: 'm;
    fn is_connected<'m>(&'m mut self) -> Self::IsConnectedFuture<'m> {
        async move { Ok(self.socket.is_some()) }
    }

    fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.socket.take();
        Ok(())
    }
}

impl embedded_io::asynch::Read for StdTcpClientSocket {
    type ReadFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            if let Some(connection) = &mut self.socket {
                connection.read(buf)
            } else {
                Err(std::io::ErrorKind::NotConnected.into())
            }
        }
    }
}

impl embedded_io::asynch::Write for StdTcpClientSocket {
    type WriteFuture<'m> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'm;

    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            if let Some(connection) = &mut self.socket {
                connection.write(buf)
            } else {
                Err(std::io::ErrorKind::NotConnected.into())
            }
        }
    }

    type FlushFuture<'m> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move { Ok(()) }
    }
}
