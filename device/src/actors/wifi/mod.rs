use crate::{
    kernel::actor::{Actor, Address},
    traits::{
        ip::{IpAddress, IpProtocol, SocketAddress},
        tcp::{TcpError, TcpStack},
        wifi::{Join, JoinError, WifiSupplicant},
    },
};
use heapless::consts;

use core::future::Future;
use core::pin::Pin;

#[cfg(feature = "wifi+esp8266")]
pub mod esp8266;

/// Actor messages handled by network adapter actors
pub enum AdapterRequest<'m> {
    Join(Join<'m>),
    Open,
    Connect(u8, IpProtocol, SocketAddress),
    Send(u8, &'m [u8]),
    Recv(u8, &'m mut [u8]),
    Close(u8),
}

/// Actor responses returned by network adapter actors
pub enum AdapterResponse {
    Join(Result<IpAddress, JoinError>),
    Open(u8),
    Connect(Result<(), TcpError>),
    Send(Result<usize, TcpError>),
    Recv(Result<usize, TcpError>),
    Close,
}

pub trait Adapter: WifiSupplicant + TcpStack<SocketHandle = u8> {}

/// Wrapper for a Wifi adapter.
#[derive(Clone, Copy)]
pub struct WifiAdapter<'a, A>
where
    A: Adapter + 'static,
{
    address: Address<'a, AdapterActor<A>>,
}

impl<'a, A> WifiAdapter<'a, A>
where
    A: Adapter + 'static,
{
    pub fn new(address: Address<'a, AdapterActor<A>>) -> Self {
        Self { address }
    }

    pub async fn join<'m>(&'m self, join_info: Join<'m>) -> Result<IpAddress, JoinError> {
        self.address
            .request(AdapterRequest::Join(join_info))
            .unwrap()
            .await
            .join()
    }

    pub async fn socket<'m>(&'m self) -> Socket<'a, A> {
        let handle = self
            .address
            .request(AdapterRequest::Open)
            .unwrap()
            .await
            .open();
        Socket::new(self.address, handle)
    }
}

/// A Socket type for connecting to a network endpoint + sending and receiving data.
#[derive(Clone, Copy)]
pub struct Socket<'a, A>
where
    A: Adapter + 'static,
{
    address: Address<'a, AdapterActor<A>>,
    handle: u8,
}

impl<'a, A> Socket<'a, A>
where
    A: Adapter + 'static,
{
    pub fn new(address: Address<'a, AdapterActor<A>>, handle: u8) -> Self {
        Self { address, handle }
    }

    pub async fn connect<'m>(
        &'m self,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Result<(), TcpError> {
        self.address
            .request(AdapterRequest::Connect(self.handle, proto, dst))
            .unwrap()
            .await
            .connect()
    }

    pub async fn send<'m>(&'m self, buf: &'m [u8]) -> Result<usize, TcpError> {
        self.address
            .request(AdapterRequest::Send(self.handle, buf))
            .unwrap()
            .await
            .send()
    }

    pub async fn recv<'m>(&'m self, buf: &'m mut [u8]) -> Result<usize, TcpError> {
        self.address
            .request(AdapterRequest::Recv(self.handle, buf))
            .unwrap()
            .await
            .recv()
    }

    pub async fn close<'m>(&'m self) {
        self.address
            .request(AdapterRequest::Close(self.handle))
            .unwrap()
            .await;
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

    fn send(self) -> Result<usize, TcpError> {
        match self {
            AdapterResponse::Send(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn recv(self) -> Result<usize, TcpError> {
        match self {
            AdapterResponse::Recv(result) => result,
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
    type MessageQueueSize<'m>
    where
        N: 'm,
    = consts::U4;

    #[rustfmt::skip]
    type Message<'m> where N: 'm = AdapterRequest<'m>;
    type Response = AdapterResponse;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.driver.replace(config);
    }

    #[rustfmt::skip]
    type OnStartFuture<'m> where N: 'm = impl Future<Output = ()> + 'm;
    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move {}
    }

    #[rustfmt::skip]
    type OnMessageFuture<'m> where N: 'm = impl Future<Output = Self::Response> + 'm;
    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let this = unsafe { self.get_unchecked_mut() };
            let driver = this.driver.as_mut().unwrap();
            match message {
                AdapterRequest::Join(join) => AdapterResponse::Join(driver.join(join).await),
                AdapterRequest::Open => AdapterResponse::Open(driver.open().await),
                AdapterRequest::Connect(handle, proto, addr) => {
                    AdapterResponse::Connect(driver.connect(handle, proto, addr).await)
                }
                AdapterRequest::Send(handle, buf) => {
                    AdapterResponse::Send(driver.write(handle, buf).await)
                }
                AdapterRequest::Recv(handle, buf) => {
                    AdapterResponse::Recv(driver.read(handle, buf).await)
                }
                AdapterRequest::Close(handle) => {
                    driver.close(handle).await;
                    AdapterResponse::Close
                }
            }
        }
    }
}
