use crate::drivers::tcp::smoltcp::{SmolSocketHandle, SmolTcpStack};
use crate::traits::ip::{IpProtocol, SocketAddress};
use crate::traits::tcp::{TcpError, TcpStack};
use crate::{Actor, ActorContext, ActorSpawner, Address, Inbox, Package};
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
    Open(Result<SmolSocketHandle, TcpError>),
    Connect(Result<(), TcpError>),
    Write(Result<usize, TcpError>),
    Read(Result<usize, TcpError>),
    Close,
}

impl SmolResponse {
    fn open(self) -> Result<SmolSocketHandle, TcpError> {
        match self {
            SmolResponse::Open(Ok(handle)) => Ok(handle),
            _ => Err(TcpError::OpenError),
        }
    }

    fn connect(self) -> Result<(), TcpError> {
        match self {
            SmolResponse::Connect(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn write(self) -> Result<usize, TcpError> {
        match self {
            SmolResponse::Write(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn read(self) -> Result<usize, TcpError> {
        match self {
            SmolResponse::Read(result) => result,
            _ => panic!("unexpected response type"),
        }
    }

    fn close(self) {
        match self {
            SmolResponse::Close => (),
            _ => panic!("unexpected response type"),
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
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        SmolRequest::Initialize => {
                            defmt::info!("initializing tcp");
                            unsafe { self.initialize() };
                            SmolResponse::Initialized
                        }
                        SmolRequest::Open => SmolResponse::Open(self.open().await),
                        SmolRequest::Connect(handle, proto, addr) => {
                            SmolResponse::Connect(self.connect(*handle, *proto, *addr).await)
                        }
                        SmolRequest::Write(handle, buf) => {
                            SmolResponse::Write(self.write(*handle, buf).await)
                        }
                        SmolRequest::Read(handle, buf) => {
                            SmolResponse::Read(self.read(*handle, buf).await)
                        }
                        SmolRequest::Close(handle) => {
                            self.close(*handle).await;
                            SmolResponse::Close
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}

// Feels like there could be a blanket impl for TcpStack-impling actors to their Address<_>.
//
impl<'a, 'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> TcpStack
    for Address<'a, SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>>
{
    type SocketHandle =
        <SmolTcpStack<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE> as TcpStack>::SocketHandle;

    #[rustfmt::skip]
    type OpenFuture<'m> where 'a: 'm = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm;

    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            self.request(SmolRequest::Open)
                .unwrap()
                .await
                .unwrap()
                .open()
        }
    }

    #[rustfmt::skip]
    type ConnectFuture<'m> where 'a: 'm  =  impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            self.request(SmolRequest::Connect(handle, proto, dst))
                .unwrap()
                .await
                .unwrap()
                .connect()
        }
    }

    #[rustfmt::skip]
    type WriteFuture<'m> where 'a: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.request(SmolRequest::Write(handle, buf))
                .unwrap()
                .await
                .unwrap()
                .write()
        }
    }

    #[rustfmt::skip]
    type ReadFuture<'m> where 'a: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            self.request(SmolRequest::Read(handle, buf))
                .unwrap()
                .await
                .unwrap()
                .read()
        }
    }

    #[rustfmt::skip]
    type CloseFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.request(SmolRequest::Close(handle)).unwrap().await;
        }
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
