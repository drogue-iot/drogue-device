/*
 * MIT License
 *
 * Copyright (c) [2022] [Ondrej Babec <ond.babec@gmail.com>]
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publishistribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIMAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use crate::actors::net::ConnectionFactory;
use crate::actors::socket::Socket;
use crate::actors::tcp::TcpActor;
use crate::traits::ip::{IpAddress, IpAddressV4, IpProtocol, SocketAddress};
use crate::Address;
use core::future::Future;
use core::ops::Range;
use rust_mqtt::packet::v5::reason_codes::ReasonCode;

use crate::traits::tcp;
use crate::traits::tcp::TcpStack;
use rust_mqtt::network::network_trait::{NetworkConnection, NetworkConnectionFactory};

pub struct DrogueNetwork<A>
where
    A: TcpActor + 'static,
{
    socket: Socket<A>,
}

impl<A> DrogueNetwork<A>
where
    A: TcpActor + 'static,
{
    fn new(socket: Socket<A>) -> Self {
        Self { socket }
    }
}

impl<A> NetworkConnection for DrogueNetwork<A>
where
    A: TcpActor + 'static,
{
    type WriteFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), ReasonCode>> + 'm;

    type ReadFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<usize, ReasonCode>> + 'm;

    type CloseFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), ReasonCode>> + 'm;

    fn send(&'m mut self, buffer: &'m mut [u8], len: usize) -> Self::WriteFuture<'m> {
        async move {
            self.socket
                .write(&buffer[0..len])
                .await
                .map_err(|_| ReasonCode::NetworkError)
                .map(|_| ())
        }
    }

    fn receive(&'m mut self, buffer: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            self.socket
                .read(buffer)
                .await
                .map_err(|_| ReasonCode::NetworkError)
        }
    }

    fn close<'m>(mut self) -> Self::CloseFuture<'m> {
        async move {
            self.socket
                .close()
                .await
                .map_err(|_| ReasonCode::NetworkError)
        }
    }
}

pub struct DrogueConnectionFactory<A>
where
    A: TcpActor + 'static,
{
    network: Address<A>,
}

impl<A> DrogueConnectionFactory<A>
where
    A: TcpActor + 'static,
{
    pub fn new(network: Address<A>) -> Self {
        Self { network }
    }
}

impl<A> NetworkConnectionFactory for DrogueConnectionFactory<A>
where
    A: TcpActor + 'static,
{
    type Connection = DrogueNetwork<A>;

    type ConnectionFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Self::Connection, ReasonCode>> + 'm;

    fn connect<'m>(&'m mut self, ip: [u8; 4], port: u16) -> Self::ConnectionFuture<'m> {
        async move {
            let mut socket = Socket::new(self.network.clone(), self.network.open().await.unwrap());

            match socket
                .connect(
                    IpProtocol::Tcp,
                    SocketAddress::new(IpAddress::new_v4(ip[0], ip[1], ip[2], ip[3]), port),
                )
                .await
            {
                Ok(_) => {
                    trace!("Connection established");
                    Ok(DrogueNetwork::new(socket))
                }
                Err(e) => {
                    warn!("Error creating connection:");
                    socket.close().await.map_err(|e| ReasonCode::NetworkError)?;
                    Err(ReasonCode::NetworkError)
                }
            }
        }
    }
}
