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

use crate::network::socket::Socket;
use core::future::Future;
use rust_mqtt::packet::v5::reason_codes::ReasonCode;

use crate::traits::tcp::TcpStack;
use rust_mqtt::network::NetworkConnection;

pub struct DrogueNetwork<A>
where
    A: TcpStack + Clone + 'static,
{
    socket: Socket<A>,
}

impl<A> DrogueNetwork<A>
where
    A: TcpStack + Clone + 'static,
{
    pub fn new(socket: Socket<A>) -> Self {
        Self { socket }
    }
}

impl<A> NetworkConnection for DrogueNetwork<A>
where
    A: TcpStack + Clone + 'static,
{
    type SendFuture<'m>
    = impl Future<Output = Result<(), ReasonCode>> + 'm where Self: 'm;

    type ReceiveFuture<'m>
    = impl Future<Output = Result<usize, ReasonCode>> + 'm where Self: 'm;

    type CloseFuture<'m> = impl Future<Output = Result<(), ReasonCode>> + 'm;

    fn send<'m>(&'m mut self, buffer: &'m [u8]) -> Self::SendFuture<'m> {
        async move {
            self.socket
                .write(buffer)
                .await
                .map_err(|_| ReasonCode::NetworkError)
                .map(|_| ())
        }
    }

    fn receive<'m>(&'m mut self, buffer: &'m mut [u8]) -> Self::ReceiveFuture<'m> {
        async move {
            self.socket
                .read(buffer)
                .await
                .map_err(|_| ReasonCode::NetworkError)
        }
    }

    fn close<'m>(self) -> Self::CloseFuture<'m> {
        async move {
            self.socket
                .close()
                .await
                .map_err(|_| ReasonCode::NetworkError)
        }
    }
}
