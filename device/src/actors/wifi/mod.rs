use crate::{
    kernel::actor::{Actor, Address, Inbox},
    traits::{
        ip::{IpAddress, IpProtocol, SocketAddress},
        tcp::{TcpError, TcpStack},
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
    Open(u8),
    Connect(Result<(), TcpError>),
    Write(Result<usize, TcpError>),
    Read(Result<usize, TcpError>),
    Close,
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

impl<'a, A> TcpStack for Address<'a, AdapterActor<A>>
where
    A: Adapter + 'static,
{
    type SocketHandle = A::SocketHandle;

    #[rustfmt::skip]
    type OpenFuture<'m> where 'a: 'm = impl Future<Output = Self::SocketHandle> + 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            self.request(AdapterRequest::Open)
                .unwrap()
                .await
                .unwrap()
                .open()
        }
    }

    #[rustfmt::skip]
    type ConnectFuture<'m> where 'a: 'm, A: 'm =  impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            self.request(AdapterRequest::Connect(handle, proto, dst))
                .unwrap()
                .await
                .unwrap()
                .connect()
        }
    }

    #[rustfmt::skip]
    type WriteFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.request(AdapterRequest::Write(handle, buf))
                .unwrap()
                .await
                .unwrap()
                .write()
        }
    }

    #[rustfmt::skip]
    type ReadFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            self.request(AdapterRequest::Read(handle, buf))
                .unwrap()
                .await
                .unwrap()
                .read()
        }
    }

    #[rustfmt::skip]
    type CloseFuture<'m> where 'a: 'm, A: 'm = impl Future<Output = ()> + 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.request(AdapterRequest::Close(handle)).unwrap().await;
        }
    }
}

impl AdapterResponse {
    fn open(self) -> u8 {
        match self {
            AdapterResponse::Open(handle) => handle,
            _ => panic!("unexpected response type"),
        }
    }

    fn join(self) -> Result<IpAddress, JoinError> {
        match self {
            AdapterResponse::Join(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn connect(self) -> Result<(), TcpError> {
        match self {
            AdapterResponse::Connect(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn write(self) -> Result<usize, TcpError> {
        match self {
            AdapterResponse::Write(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn read(self) -> Result<usize, TcpError> {
        match self {
            AdapterResponse::Read(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn close(self) {
        match self {
            AdapterResponse::Close => (),
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
                        AdapterRequest::Open => AdapterResponse::Open(driver.open().await),
                        AdapterRequest::Connect(handle, proto, addr) => {
                            AdapterResponse::Connect(driver.connect(*handle, *proto, *addr).await)
                        }
                        AdapterRequest::Write(handle, buf) => {
                            AdapterResponse::Write(driver.write(*handle, buf).await)
                        }
                        AdapterRequest::Read(handle, buf) => {
                            AdapterResponse::Read(driver.read(*handle, buf).await)
                        }
                        AdapterRequest::Close(handle) => {
                            driver.close(*handle).await;
                            AdapterResponse::Close
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}
