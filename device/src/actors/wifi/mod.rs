use super::tcp::{TcpActor, TcpResponse};
use crate::{
    traits::{
        ip::{IpAddress, IpProtocol, SocketAddress},
        tcp::TcpStack,
        wifi::{Join, JoinError, WifiSupplicant},
    },
    {Actor, Address, Inbox},
};
use core::future::Future;

#[cfg(feature = "wifi+esp8266")]
pub mod esp8266;

/// Actor messages handled by network adapter actors
pub enum AdapterRequest<'m> {
    Join(Join<'m>),
    Open,
    Connect(u8, IpProtocol, SocketAddress),
    Write(u8, &'m [u8]),
    Read(u8, &'m mut [u8]),
    Close(u8),
}

/// Actor responses returned by network adapter actors
pub enum AdapterResponse {
    Join(Result<IpAddress, JoinError>),
    Tcp(TcpResponse<u8>),
}

impl Into<TcpResponse<u8>> for Option<AdapterResponse> {
    fn into(self) -> TcpResponse<u8> {
        match self {
            Some(AdapterResponse::Tcp(r)) => r,
            _ => panic!("cannot convert response to tcp response"),
        }
    }
}

pub trait Adapter: WifiSupplicant + TcpStack<SocketHandle = u8> {}

impl<A> WifiSupplicant for Address<AdapterActor<A>>
where
    A: Adapter + 'static,
{
    type JoinFuture<'m> = impl Future<Output = Result<IpAddress, JoinError>> + 'm;
    fn join<'m>(&'m mut self, join: Join<'m>) -> Self::JoinFuture<'m> {
        async move {
            self.request(AdapterRequest::Join(join))
                .unwrap()
                .await
                .unwrap()
                .join()
        }
    }
}

impl<N: Adapter> TcpActor for AdapterActor<N> {
    type SocketHandle = u8;

    fn open<'m>() -> AdapterRequest<'m>
    where
        N: 'm,
    {
        AdapterRequest::Open
    }
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> AdapterRequest<'m>
    where
        N: 'm,
    {
        AdapterRequest::Connect(handle, proto, dst)
    }
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> AdapterRequest<'m>
    where
        N: 'm,
    {
        AdapterRequest::Write(handle, buf)
    }
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> AdapterRequest<'m>
    where
        N: 'm,
    {
        AdapterRequest::Read(handle, buf)
    }
    fn close<'m>(handle: Self::SocketHandle) -> AdapterRequest<'m>
    where
        N: 'm,
    {
        AdapterRequest::Close(handle)
    }

    fn into_response(response: Self::Response) -> Option<TcpResponse<u8>> {
        match response {
            Some(AdapterResponse::Tcp(r)) => Some(r),
            _ => None,
        }
    }
}

impl AdapterResponse {
    fn join(self) -> Result<IpAddress, JoinError> {
        match self {
            AdapterResponse::Join(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn tcp(self) -> TcpResponse<u8> {
        match self {
            AdapterResponse::Tcp(r) => r,
            _ => panic!("unexpected response type"),
        }
    }
}

pub struct AdapterActor<N: Adapter> {
    driver: N,
}

impl<N: Adapter> AdapterActor<N> {
    pub fn new(driver: N) -> Self {
        Self { driver }
    }
}

impl<N: Adapter> Actor for AdapterActor<N> {
    type Message<'m> = AdapterRequest<'m> where N: 'm;
    type Response = Option<AdapterResponse>;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where N: 'm, M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        AdapterRequest::Join(join) => {
                            AdapterResponse::Join(self.driver.join(*join).await)
                        }
                        AdapterRequest::Open => {
                            AdapterResponse::Tcp(TcpResponse::Open(self.driver.open().await))
                        }
                        AdapterRequest::Connect(handle, proto, addr) => AdapterResponse::Tcp(
                            TcpResponse::Connect(self.driver.connect(*handle, *proto, *addr).await),
                        ),
                        AdapterRequest::Write(handle, buf) => AdapterResponse::Tcp(
                            TcpResponse::Write(self.driver.write(*handle, buf).await),
                        ),
                        AdapterRequest::Read(handle, buf) => AdapterResponse::Tcp(
                            TcpResponse::Read(self.driver.read(*handle, buf).await),
                        ),
                        AdapterRequest::Close(handle) => {
                            let r = self.driver.close(*handle).await;
                            AdapterResponse::Tcp(TcpResponse::Close(r))
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}
