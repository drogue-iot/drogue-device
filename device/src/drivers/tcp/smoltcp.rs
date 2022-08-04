use core::future::Future;
use embassy_net::{
    tcp::{Error as SocketError, TcpSocket},
    Device, IpAddress, Ipv4Address, Ipv6Address, Stack,
};
use embedded_nal_async::{IpAddr};
use core::sync::atomic::{Ordering, AtomicBool};
use core::mem::MaybeUninit;
use core::ptr::NonNull;

pub struct TcpClient<'d, D: Device, const N: usize, const TX_SZ: usize = 1024, const RX_SZ: usize = 1024> {
    stack: &'d Stack<D>,
    tx: &'d Pool<[u8; TX_SZ], N>,
    rx: &'d Pool<[u8; RX_SZ], N>,
}

impl<'d, D: Device, const N: usize, const TX_SZ: usize, const RX_SZ: usize> TcpClient<'d, D, N, TX_SZ, RX_SZ> {
    pub fn new(stack: &'d Stack<D>, tx: &'d Pool<[u8; TX_SZ], N>, rx: &'d Pool<[u8; RX_SZ], N>) -> Self {
        Self {
            stack,
            tx,
            rx,
        }
    }
}

impl<'d, D: Device, const N: usize, const TX_SZ: usize, const RX_SZ: usize> embedded_nal_async::TcpConnect for TcpClient<'d, D, N, TX_SZ, RX_SZ> {
    type Error = SocketError;
    type Connection<'m> = TcpConnection<'m, N, TX_SZ, RX_SZ> where Self: 'm;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection<'m>, Self::Error>> + 'm
    where
        Self: 'm;

    fn connect<'m>(&'m self, remote: embedded_nal_async::SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            let addr: IpAddress = match remote.ip() {
                IpAddr::V4(addr) => IpAddress::Ipv4(Ipv4Address::from_bytes(&addr.octets())),
                IpAddr::V6(addr) => IpAddress::Ipv6(Ipv6Address::from_bytes(&addr.octets())),
            };
            let remote_endpoint = (addr, remote.port());
            let mut socket = TcpConnection::new(&self.stack, self.tx, self.rx)?;
            socket.socket.connect(remote_endpoint).await.map_err(|_| SocketError::ConnectionReset)?;
            Ok(socket)
        }
    }
}

pub struct TcpConnection<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> {
    socket: TcpSocket<'d>,
    tx: &'d Pool<[u8; TX_SZ], N>,
    rx: &'d Pool<[u8; RX_SZ], N>,
    txb: NonNull<[u8; TX_SZ]>,
    rxb: NonNull<[u8; RX_SZ]>,
}

impl<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> TcpConnection<'d, N, TX_SZ, RX_SZ> {
    pub fn new<D: Device>(stack: &'d Stack<D>, tx: &'d Pool<[u8; TX_SZ], N>, rx: &'d Pool<[u8; RX_SZ], N>) -> Result<Self, SocketError> {
        let mut txb = tx.alloc().ok_or(SocketError::ConnectionReset)?;
        let mut rxb = rx.alloc().ok_or(SocketError::ConnectionReset)?;
        Ok(Self {
            socket: unsafe { TcpSocket::new(stack, rxb.as_mut(), txb.as_mut()) },
            tx,
            rx,
            txb,
            rxb,
        })
    }
}

impl<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> Drop for TcpConnection<'d, N, TX_SZ, RX_SZ> {
    fn drop(&mut self) {
        unsafe {
            self.socket.close();
            self.rx.free(self.rxb);
            self.tx.free(self.txb);
        }
    }
}

impl<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> embedded_io::Io for TcpConnection<'d, N, TX_SZ, RX_SZ> {
    type Error = SocketError;
}

impl<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> embedded_io::asynch::Read for TcpConnection<'d, N, TX_SZ, RX_SZ> {
    type ReadFuture<'a> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'a;

    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Self::ReadFuture<'a> {
        self.socket.read(buf)
    }
}

impl<'d, const N: usize, const TX_SZ: usize, const RX_SZ: usize> embedded_io::asynch::Write for TcpConnection<'d, N, TX_SZ, RX_SZ> {
    type WriteFuture<'a> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'a;

    fn write<'a>(&'a mut self, buf: &'a [u8]) -> Self::WriteFuture<'a> {
        self.socket.write(buf)
    }

    type FlushFuture<'a> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    fn flush<'a>(&'a mut self) -> Self::FlushFuture<'a> {
        self.socket.flush()
    }
}

pub struct Pool<T, const N: usize>
{
    used: [AtomicBool; N],
    data: MaybeUninit<[T; N]>,
}

impl<T, const N: usize> Pool<T, N>
{
    pub const fn new() -> Self {
        const VALUE: AtomicBool = AtomicBool::new(false);
        Self {
            used: [VALUE; N],
            data: MaybeUninit::uninit(),
        }
    }
}

impl<T, const N: usize> Pool<T, N>
{
    fn alloc(&self) -> Option<NonNull<T>> {
        for n in 0..N {
            if self.used[n].swap(true, Ordering::SeqCst) == false {
                let origin = self.data.as_ptr() as *mut T;
                return Some(unsafe { NonNull::new_unchecked(origin.add(n)) })
            }
        }
        None
    }

    /// safety: p must be a pointer obtained from self.alloc that hasn't been freed yet.
    unsafe fn free(&self, p: NonNull<T>) {
        let origin = self.data.as_ptr() as *mut T;
        let n = p.as_ptr().offset_from(origin);
        assert!(n >= 0);
        assert!((n as usize) < N);
        self.used[n as usize].store(false, Ordering::SeqCst);
    }
}

