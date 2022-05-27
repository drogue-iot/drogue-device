use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Waker;
use core::task::{Context, Poll};
use embassy_net::tcp::TcpSocket;
use heapless::spsc::Queue;
use heapless::Vec;

#[derive(Debug, Copy, Clone)]
pub(crate) struct PoolHandle(usize);

pub(crate) struct SocketPool<
    'buffer,
    const POOL_SIZE: usize,
    const BACKLOG: usize,
    const BUF_SIZE: usize,
> {
    buffers: UnsafeCell<[([u8; BUF_SIZE], [u8; BUF_SIZE]); POOL_SIZE]>,
    sockets: UnsafeCell<Vec<TcpSocket<'buffer>, POOL_SIZE>>,
    tokens: RefCell<[Option<()>; POOL_SIZE]>,
    wakers: RefCell<Queue<Waker, BACKLOG>>,
    _marker: PhantomData<&'buffer ()>,
}

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize>
    SocketPool<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    pub(crate) fn new() -> Self {
        Self {
            buffers: UnsafeCell::new([([0; BUF_SIZE], [0; BUF_SIZE]); POOL_SIZE]),
            sockets: UnsafeCell::new(Vec::new()),
            tokens: RefCell::new([None; POOL_SIZE]),
            wakers: RefCell::new(Queue::new()),
            _marker: PhantomData,
        }
    }

    pub(crate) fn initialize(&self) {
        info!("initializing socket pool");
        unsafe {
            for (rx_buf, tx_buf) in (&mut *self.buffers.get()).iter_mut() {
                let socket = TcpSocket::new(rx_buf, tx_buf);
                (&mut *self.sockets.get()).push(socket).ok();
            }
        }
    }

    pub(crate) fn get_socket(&self, handle: PoolHandle) -> Result<&mut TcpSocket<'buffer>, ()> {
        unsafe { (&mut *self.sockets.get()).get_mut(handle.0).ok_or(()) }
    }

    pub(crate) async fn borrow(&self) -> Result<PoolHandle, ()> {
        BorrowFuture::new(self).await
    }

    pub(crate) fn unborrow(&self, handle: PoolHandle) {
        self.tokens.borrow_mut()[handle.0].take();
    }

    fn poll_borrow(&self, waker: &Waker, already_waiting: bool) -> Poll<Result<PoolHandle, ()>> {
        let mut tokens = self.tokens.borrow_mut();
        let available = tokens
            .iter()
            .enumerate()
            .filter(|e| matches!(e, (_, None)))
            .next();

        if let Some((index, _)) = available {
            tokens[index].replace(());
            Poll::Ready(Ok(PoolHandle(index)))
        } else {
            if !already_waiting {
                return match self.wakers.borrow_mut().enqueue(waker.clone()) {
                    Ok(_) => Poll::Pending,
                    Err(_) => Poll::Ready(Err(())),
                };
            }
            Poll::Pending
        }
    }
}

pub(crate) struct BorrowFuture<
    'a,
    'buffer,
    const POOL_SIZE: usize,
    const BACKLOG: usize,
    const BUF_SIZE: usize,
> {
    waiting: bool,
    pool: &'a SocketPool<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>,
}
impl<'a, 'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize>
    BorrowFuture<'a, 'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    fn new(pool: &'a SocketPool<'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>) -> Self {
        Self {
            pool,
            waiting: false,
        }
    }
}

impl<'buffer, const POOL_SIZE: usize, const BACKLOG: usize, const BUF_SIZE: usize> Future
    for BorrowFuture<'_, 'buffer, POOL_SIZE, BACKLOG, BUF_SIZE>
{
    type Output = Result<PoolHandle, ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = self.pool.poll_borrow(cx.waker(), self.waiting);
        if result.is_pending() {
            self.waiting = true;
        }
        result
    }
}
