use crate::actors::tcp::{TcpActor, TcpResponse};
use crate::drivers::tcp::smoltcp::{SmolSocketHandle, SmolTcpStack};
use crate::traits::ip::{IpProtocol, SocketAddress};
use crate::traits::tcp::TcpStack;
use crate::{Actor, Address, Inbox};
use core::future::Future;

/// Actor messages handled by network adapter actors
pub enum SmolRequest<'m> {
    Initialize,
    Open,
    Connect(SmolSocketHandle, IpProtocol, SocketAddress),
    Write(SmolSocketHandle, &'m [u8]),
    Read(SmolSocketHandle, &'m mut [u8]),
    Close(SmolSocketHandle),
}

/// Actor responses returned by network adapter actors
pub enum SmolResponse {
    Initialized,
    Tcp(TcpResponse<SmolSocketHandle>),
}

impl SmolResponse {
    fn tcp(self) -> TcpResponse<SmolSocketHandle> {
        match self {
            SmolResponse::Tcp(r) => r,
            _ => panic!("unexpected response type"),
        }
    }
}

impl Into<TcpResponse<SmolSocketHandle>> for Option<SmolResponse> {
    fn into(self) -> TcpResponse<SmolSocketHandle> {
        match self {
            Some(SmolResponse::Tcp(r)) => r,
            _ => panic!("cannot convert response to tcp response"),
        }
    }
}

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> Actor
    for SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    #[rustfmt::skip]
    type Message<'m> where 'buffer: 'm = SmolRequest<'m>;
    type Response = Option<SmolResponse>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                let mut m = inbox.next().await;
                let response = match m.message() {
                    SmolRequest::Initialize => {
                        unsafe { self.initialize() };
                        SmolResponse::Initialized
                    }
                    SmolRequest::Open => SmolResponse::Tcp(TcpResponse::Open(self.open().await)),
                    SmolRequest::Connect(handle, proto, addr) => SmolResponse::Tcp(
                        TcpResponse::Connect(self.connect(*handle, *proto, *addr).await),
                    ),
                    SmolRequest::Write(handle, buf) => {
                        SmolResponse::Tcp(TcpResponse::Write(self.write(*handle, buf).await))
                    }
                    SmolRequest::Read(handle, buf) => {
                        SmolResponse::Tcp(TcpResponse::Read(self.read(*handle, buf).await))
                    }
                    SmolRequest::Close(handle) => {
                        let r = self.close(*handle).await;
                        SmolResponse::Tcp(TcpResponse::Close(r))
                    }
                };
                m.set_response(Some(response));
            }
        }
    }
}

impl<'a, 'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize>
    TcpActor<SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>>
    for SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type SocketHandle = SmolSocketHandle;

    fn open<'m>() -> SmolRequest<'m> {
        SmolRequest::Open
    }
    fn connect<'m>(
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> SmolRequest<'m> {
        SmolRequest::Connect(handle, proto, dst)
    }
    fn write<'m>(handle: Self::SocketHandle, buf: &'m [u8]) -> SmolRequest<'m> {
        SmolRequest::Write(handle, buf)
    }
    fn read<'m>(handle: Self::SocketHandle, buf: &'m mut [u8]) -> SmolRequest<'m> {
        SmolRequest::Read(handle, buf)
    }
    fn close<'m>(handle: Self::SocketHandle) -> SmolRequest<'m> {
        SmolRequest::Close(handle)
    }
}

pub struct EmbassyNetTask;

impl Actor for EmbassyNetTask {
    type Message<'m> = ();

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move { embassy_net::run().await }
    }
}
