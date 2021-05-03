use core::{
    cell::{RefCell, UnsafeCell},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use embassy::util::{critical_section, AtomicWaker};
use heapless::spsc::{Consumer, Producer, Queue};
pub use heapless::{consts, ArrayLength};

struct ChannelInner<T, N: ArrayLength<T>> {
    queue: UnsafeCell<Queue<T, N>>,
    sender_waker: AtomicWaker,
    receiver_waker: AtomicWaker,
}

impl<T, N: ArrayLength<T>> Default for ChannelInner<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, N: ArrayLength<T>> ChannelInner<T, N> {
    pub fn new() -> Self {
        Self {
            queue: UnsafeCell::new(Queue::new()),
            sender_waker: AtomicWaker::new(),
            receiver_waker: AtomicWaker::new(),
        }
    }

    fn register_receiver(&self, waker: &Waker) {
        self.receiver_waker.register(&waker);
    }

    fn register_sender(&self, waker: &Waker) {
        self.sender_waker.register(&waker);
    }

    fn wake_sender(&self) {
        self.sender_waker.wake();
    }

    fn wake_receiver(&self) {
        self.receiver_waker.wake();
    }

    fn split(&mut self) -> (ChannelSender<'_, T, N>, ChannelReceiver<'_, T, N>) {
        let (sender, receiver) = unsafe { (&mut *self.queue.get()).split() };
        (
            ChannelSender::new(sender, self),
            ChannelReceiver::new(receiver, self),
        )
    }
}

pub struct Channel<T, N: ArrayLength<T>> {
    inner: ChannelInner<T, N>,
}

impl<T, N: ArrayLength<T>> Default for Channel<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, N: ArrayLength<T>> Channel<T, N> {
    pub fn new() -> Self {
        let inner = ChannelInner::new();
        Self { inner }
    }

    pub fn split(&mut self) -> (ChannelSender<'_, T, N>, ChannelReceiver<'_, T, N>) {
        self.inner.split()
    }
}

pub struct ChannelSender<'a, T, N: ArrayLength<T>> {
    inner: &'a ChannelInner<T, N>,
    producer: RefCell<Producer<'a, T, N>>,
}

#[derive(Debug)]
pub enum ChannelError {
    ChannelFull,
    ChannelEmpty,
}

impl<'a, T, N: 'a + ArrayLength<T>> ChannelSender<'a, T, N> {
    fn new(producer: Producer<'a, T, N>, inner: &'a ChannelInner<T, N>) -> Self {
        Self {
            producer: RefCell::new(producer),
            inner,
        }
    }

    fn poll_enqueue(&self, cx: &mut Context<'_>, element: &mut Option<T>) -> Poll<()> {
        let mut producer = self.producer.borrow_mut();
        if producer.ready() {
            let value = element.take().unwrap();
            producer.enqueue(value).ok().unwrap();
            self.inner.wake_receiver();
            Poll::Ready(())
        } else {
            self.inner.register_sender(cx.waker());
            Poll::Pending
        }
    }

    pub fn try_send(&self, value: T) -> Result<(), ChannelError> {
        critical_section(|_| {
            let mut producer = self.producer.borrow_mut();
            producer
                .enqueue(value)
                .map_err(|_| ChannelError::ChannelFull)
                .map(|_| self.inner.wake_receiver())
        })
    }

    pub fn send<'m>(&'m self, value: T) -> ChannelSend<'m, 'a, T, N> {
        ChannelSend {
            sender: &self,
            element: Some(value),
        }
    }
}

pub struct ChannelSend<'m, 'a, T, N: 'a + ArrayLength<T>> {
    sender: &'m ChannelSender<'a, T, N>,
    element: Option<T>,
}

impl<'m, 'a, T, N: ArrayLength<T>> Unpin for ChannelSend<'m, 'a, T, N> {}

impl<'m, 'a, T, N: ArrayLength<T>> Future for ChannelSend<'m, 'a, T, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.sender.poll_enqueue(cx, &mut self.element)
    }
}

pub struct ChannelReceiver<'a, T, N: ArrayLength<T>> {
    inner: &'a ChannelInner<T, N>,
    consumer: RefCell<Consumer<'a, T, N>>,
}

impl<'a, T, N: 'a + ArrayLength<T>> ChannelReceiver<'a, T, N> {
    fn new(consumer: Consumer<'a, T, N>, inner: &'a ChannelInner<T, N>) -> Self {
        Self {
            consumer: RefCell::new(consumer),
            inner,
        }
    }

    fn poll_try_dequeue(&self) -> Poll<Option<T>> {
        if let Some(value) = self.consumer.borrow_mut().dequeue() {
            self.inner.wake_sender();
            Poll::Ready(Some(value))
        } else {
            Poll::Ready(None)
        }
    }

    fn poll_dequeue(&self, cx: &mut Context<'_>) -> Poll<T> {
        if let Some(value) = self.consumer.borrow_mut().dequeue() {
            self.inner.wake_sender();
            Poll::Ready(value)
        } else {
            self.inner.register_receiver(cx.waker());
            Poll::Pending
        }
    }

    pub fn receive<'m>(&'m self) -> ChannelReceive<'m, 'a, T, N> {
        ChannelReceive { receiver: &self }
    }
    pub fn try_receive<'m>(&'m self) -> ChannelTryReceive<'m, 'a, T, N> {
        ChannelTryReceive { receiver: &self }
    }
}

pub struct ChannelReceive<'m, 'a, T, N: ArrayLength<T>> {
    receiver: &'m ChannelReceiver<'a, T, N>,
}

impl<'m, 'a, T, N: ArrayLength<T>> Future for ChannelReceive<'m, 'a, T, N> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll_dequeue(cx)
    }
}

pub struct ChannelTryReceive<'m, 'a, T, N: ArrayLength<T>> {
    receiver: &'m ChannelReceiver<'a, T, N>,
}

impl<'m, 'a, T, N: ArrayLength<T>> Future for ChannelTryReceive<'m, 'a, T, N> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll_try_dequeue()
    }
}
