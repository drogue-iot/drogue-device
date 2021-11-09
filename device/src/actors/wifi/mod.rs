use super::tcp::{TcpActor, TcpResponse};
use crate::{
    kernel::actor::{Actor, Address, Inbox},
    traits::{
        ip::{IpAddress, IpProtocol, SocketAddress},
        tcp::TcpStack,
        wifi::{Join, JoinError, WifiSupplicant},
    },
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

impl<'a, A> WifiSupplicant for Address<'a, AdapterActor<A>>
where
    A: Adapter + 'static,
{
    #[rustfmt::skip]
    type JoinFuture<'m> where 'a: 'm = impl Future<Output = Result<IpAddress, JoinError>> + 'm;
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

impl<N: Adapter> TcpActor<AdapterActor<N>> for AdapterActor<N> {
    type SocketHandle = u8;

    fn open<'m>() -> AdapterRequest<'m> {
        AdapterRequest::Open
    }
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> AdapterRequest<'m> {
        AdapterRequest::Connect(handle, proto, dst)
    }
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> AdapterRequest<'m> {
        AdapterRequest::Write(handle, buf)
    }
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> AdapterRequest<'m> {
        AdapterRequest::Read(handle, buf)
    }
    fn close<'m>(handle: Self::SocketHandle) -> AdapterRequest<'m> {
        AdapterRequest::Close(handle)
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
    driver: Option<N>,
}

impl<N: Adapter> AdapterActor<N> {
    pub fn new() -> Self {
        Self { driver: None }
    }
}

impl<N: Adapter> Actor for AdapterActor<N> {
    type Configuration = N;

    #[rustfmt::skip]
    type Message<'m> where N: 'm = AdapterRequest<'m>;
    type Response = Option<AdapterResponse>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where N: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.driver.replace(config);
        async move {
            let driver = self.driver.as_mut().unwrap();
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        AdapterRequest::Join(join) => {
                            AdapterResponse::Join(driver.join(*join).await)
                        }
                        AdapterRequest::Open => {
                            AdapterResponse::Tcp(TcpResponse::Open(driver.open().await))
                        }
                        AdapterRequest::Connect(handle, proto, addr) => AdapterResponse::Tcp(
                            TcpResponse::Connect(driver.connect(*handle, *proto, *addr).await),
                        ),
                        AdapterRequest::Write(handle, buf) => AdapterResponse::Tcp(
                            TcpResponse::Write(driver.write(*handle, buf).await),
                        ),
                        AdapterRequest::Read(handle, buf) => {
                            AdapterResponse::Tcp(TcpResponse::Read(driver.read(*handle, buf).await))
                        }
                        AdapterRequest::Close(handle) => {
                            let r = driver.close(*handle).await;
                            AdapterResponse::Tcp(TcpResponse::Close(r))
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}
