use atomic_polyfill::{AtomicBool, Ordering};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy::util::Signal;

pub struct SignalFuture<'s, T: Send> {
    signal: &'s SignalSlot<T>,
}

impl<'s, T: Send> SignalFuture<'s, T> {
    pub fn new(signal: &'s SignalSlot<T>) -> Self {
        Self { signal }
    }

    pub fn release(&self) {
        self.signal.release();
    }
}

// impl Unpin for SignalFuture<'_, '_> {}

impl<T: Send> Future for SignalFuture<'_, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.signal.poll_wait(cx)
    }
}

impl<T: Send> Drop for SignalFuture<'_, T> {
    fn drop(&mut self) {
        self.signal.release();
    }
}

pub struct SignalSlot<T: Send> {
    free: AtomicBool,
    signal: Signal<T>,
}

impl<T: Send> SignalSlot<T> {
    pub fn acquire(&self) -> bool {
        if self.free.swap(false, Ordering::AcqRel) {
            self.signal.reset();
            true
        } else {
            false
        }
    }

    pub fn poll_wait(&self, cx: &mut Context<'_>) -> Poll<T> {
        self.signal.poll_wait(cx)
    }

    pub fn signal(&self, value: T) {
        self.signal.signal(value)
    }

    pub fn release(&self) {
        self.free.store(true, Ordering::Release)
    }
}

impl<T: Send> Default for SignalSlot<T> {
    fn default() -> Self {
        Self {
            free: AtomicBool::new(true),
            signal: Signal::new(),
        }
    }
}
