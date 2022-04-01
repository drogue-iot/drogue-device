use crate::traits::{ip::*, tcp::*};
use core::future::Future;
use embassy::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};

type DriverMutex = NoopRawMutex;

pub struct TcpStackState<T: TcpStack> {
    network: Mutex<DriverMutex, Option<T>>,
}

impl<T: TcpStack> TcpStackState<T> {
    pub const fn new() -> Self {
        Self {
            network: Mutex::new(None),
        }
    }

    pub fn initialize<'a>(&'a self, network: T) -> SharedTcpStack<'a, T> {
        if let Ok(mut guard) = self.network.try_lock() {
            guard.replace(network);
        }
        SharedTcpStack {
            handle: &self.network,
        }
    }
}

unsafe impl<T: TcpStack> Sync for TcpStackState<T> {}

pub struct SharedTcpStack<'a, T: TcpStack> {
    handle: &'a Mutex<DriverMutex, Option<T>>,
}

impl<'a, T: TcpStack> Clone for SharedTcpStack<'a, T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
        }
    }
}

impl<'a, T: TcpStack> TcpStack for SharedTcpStack<'a, T> {
    type SocketHandle = T::SocketHandle;

    type OpenFuture<'m> = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm
    where
        Self: 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move { self.handle.lock().await.as_mut().unwrap().open().await }
    }

    type ConnectFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .connect(handle, proto, dst)
                .await
        }
    }

    type WriteFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .write(handle, buf)
                .await
        }
    }

    type ReadFuture<'m> = impl Future<Output = Result<usize, TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .read(handle, buf)
                .await
        }
    }

    type CloseFuture<'m> = impl Future<Output = Result<(), TcpError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            self.handle
                .lock()
                .await
                .as_mut()
                .unwrap()
                .close(handle)
                .await
        }
    }
}
