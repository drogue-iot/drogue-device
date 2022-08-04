use core::future::Future;
use embedded_io::adapters::FromStd;
use embedded_nal_async::*;
use std::net::TcpStream;

pub struct StdTcpClientSocket;

impl Default for StdTcpClientSocket {
    fn default() -> Self {
        Self
    }
}

impl TcpConnect for StdTcpClientSocket {
    type Error = std::io::Error;
    type Connection<'m> = FromStd<TcpStream>;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection<'m>, Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(&'m self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
                Ok(stream) => {
                    Ok(FromStd::new(stream))
                }
                Err(e) => Err(e),
            }
        }
    }
}
