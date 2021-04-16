use core::{
    cell::{RefCell, UnsafeCell},
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use embassy::util::AtomicWaker;
use heapless::spsc::{Consumer, Producer, Queue};

struct ChannelInner<'a, T, const N: usize> {
    queue: UnsafeCell<Queue<T, usize, N>>,
    producer: RefCell<Option<Producer<'a, T, usize, N>>>,
    consumer: RefCell<Option<Consumer<'a, T, usize, N>>>,
    producer_waker: AtomicWaker,
    consumer_waker: AtomicWaker,
}

impl<'a, T, const N: usize> Default for ChannelInner<'a, T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T, const N: usize> ChannelInner<'a, T, N> {
    pub fn new() -> Self {
        Self {
            queue: UnsafeCell::new(Queue::new()),
            producer: RefCell::new(None),
            consumer: RefCell::new(None),
            producer_waker: AtomicWaker::new(),
            consumer_waker: AtomicWaker::new(),
        }
    }

    fn split(&'a self) {
        let (producer, consumer) = unsafe { (&mut *self.queue.get()).split() };
        self.producer.borrow_mut().replace(producer);
        self.consumer.borrow_mut().replace(consumer);
    }

    fn poll_dequeue(&self, cx: &mut Context<'_>) -> Poll<T> {
        if let Some(value) = self.consumer.borrow_mut().as_mut().unwrap().dequeue() {
            self.producer_waker.wake();
            Poll::Ready(value)
        } else {
            self.consumer_waker.register(cx.waker());
            Poll::Pending
        }
    }

    fn poll_enqueue(&self, cx: &mut Context<'_>, element: &mut Option<T>) -> Poll<()> {
        let mut producer = self.producer.borrow_mut();
        if producer.as_mut().unwrap().ready() {
            let value = element.take().unwrap();
            producer.as_mut().unwrap().enqueue(value).ok().unwrap();
            self.consumer_waker.wake();
            Poll::Ready(())
        } else {
            self.producer_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

pub struct Channel<'a, T, const N: usize> {
    inner: ChannelInner<'a, T, N>,
}

impl<'a, T, const N: usize> Default for Channel<'a, T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T, const N: usize> Channel<'a, T, N> {
    pub fn new() -> Self {
        let inner = ChannelInner::new();
        Self { inner }
    }

    pub fn initialize(&'a self) {
        self.inner.split();
    }

    pub fn send(&'a self, value: T) -> ChannelSend<'a, T, N> {
        ChannelSend {
            inner: &self.inner,
            element: Some(value),
        }
    }
    pub fn receive(&'a self) -> ChannelReceive<'a, T, N> {
        ChannelReceive { inner: &self.inner }
    }
}

pub struct ChannelSend<'a, T, const N: usize> {
    inner: &'a ChannelInner<'a, T, N>,
    element: Option<T>,
}

impl<'a, T, const N: usize> Unpin for ChannelSend<'a, T, N> {}

impl<'a, T, const N: usize> Future for ChannelSend<'a, T, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.poll_enqueue(cx, &mut self.element)
    }
}

pub struct ChannelReceive<'a, T, const N: usize> {
    inner: &'a ChannelInner<'a, T, N>,
}

impl<'a, T, const N: usize> Future for ChannelReceive<'a, T, N> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.poll_dequeue(cx)
    }
}
