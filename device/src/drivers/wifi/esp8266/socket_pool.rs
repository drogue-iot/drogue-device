use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use heapless::{consts::*, spsc::Queue};

#[derive(PartialEq)]
enum SocketState {
    HalfClosed,
    Closed,
    Open,
    Connected,
}

impl Default for SocketState {
    fn default() -> Self {
        Self::Closed
    }
}

pub(crate) struct SocketPool {
    sockets: RefCell<[SocketState; 4]>,
    waiters: RefCell<Queue<Waker, U8>>,
}

impl SocketPool {
    pub(crate) fn new() -> Self {
        Self {
            sockets: Default::default(),
            waiters: RefCell::new(Queue::new()),
        }
    }

    pub(crate) async fn open<'a>(&'a self) -> u8 {
        OpenFuture::new(self).await
    }

    pub(crate) fn close<'a>(&'a self, socket: u8) {
        let mut sockets = self.sockets.borrow_mut();
        let index = socket as usize;
        match sockets[index] {
            SocketState::HalfClosed => {
                sockets[index] = SocketState::Closed;
            }
            SocketState::Open | SocketState::Connected => {
                sockets[index] = SocketState::HalfClosed;
            }
            SocketState::Closed => {
                // nothing
            }
        }
    }

    pub(crate) fn is_closed<'a>(&'a self, socket: u8) -> bool {
        let sockets = self.sockets.borrow();
        let index = socket as usize;
        sockets[index] == SocketState::Closed || sockets[index] == SocketState::HalfClosed
    }

    fn poll_open(&self, waker: &Waker, waiting: bool) -> Poll<u8> {
        let mut sockets = self.sockets.borrow_mut();
        let available = sockets
            .iter()
            .enumerate()
            .filter(|e| matches!(e, (_, SocketState::Closed)))
            .next();

        if let Some((index, _)) = available {
            sockets[index] = SocketState::Open;
            Poll::Ready(index as u8)
        } else {
            if !waiting {
                self.waiters.borrow_mut().enqueue(waker.clone()).unwrap();
            }
            Poll::Pending
        }
    }
}

pub(crate) struct OpenFuture<'a> {
    pool: &'a SocketPool,
    waiting: bool,
}

impl<'a> OpenFuture<'a> {
    fn new(pool: &'a SocketPool) -> Self {
        Self {
            pool,
            waiting: false,
        }
    }
}
impl<'a> Future for OpenFuture<'a> {
    type Output = u8;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = self.pool.poll_open(cx.waker(), self.waiting);
        if result.is_pending() {
            self.waiting = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn max_simultaneous_sockets() {
        let pool = SocketPool::new();
        for i in 0..100 {
            let expected = i % 4;
            if !pool.is_closed(expected) {
                pool.close(expected);
                pool.close(expected); // account for HalfClosed state
            }
            let actual = block_on(pool.open());
            assert_eq!(expected, actual);
        }
    }
}
