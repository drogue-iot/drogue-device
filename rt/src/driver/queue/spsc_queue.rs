use crate::api::queue::*;
use crate::arch::atomic;
use crate::prelude::*;
use crate::synchronization::Signal;
use core::cell::{RefCell, UnsafeCell};
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use core::task::{Context, Poll};
use heapless::{
    spsc::{Consumer, Producer, Queue as HQueue},
    ArrayLength,
};

pub struct Shared<T, N>
where
    N: ArrayLength<T> + 'static,
{
    producer: RefCell<Option<RefCell<Producer<'static, T, N>>>>,
    consumer: RefCell<Option<RefCell<Consumer<'static, T, N>>>>,

    producer_signal: Signal<()>,
    consumer_signal: Signal<()>,

    producer_state: AtomicBool,
    consumer_state: AtomicBool,

    pending_enqueue: RefCell<Option<T>>,
}

const READY_STATE: bool = false;
const BUSY_STATE: bool = true;

impl<T, N> Shared<T, N>
where
    N: ArrayLength<T> + 'static,
{
    fn new() -> Self {
        Self {
            producer: RefCell::new(None),
            consumer: RefCell::new(None),
            consumer_signal: Signal::new(),
            producer_signal: Signal::new(),
            producer_state: AtomicBool::new(READY_STATE),
            consumer_state: AtomicBool::new(READY_STATE),
            pending_enqueue: RefCell::new(None),
        }
    }

    fn set_producer(&self, producer: Producer<'static, T, N>) {
        self.producer.borrow_mut().replace(RefCell::new(producer));
    }

    fn set_consumer(&self, consumer: Consumer<'static, T, N>) {
        self.consumer.borrow_mut().replace(RefCell::new(consumer));
    }

    fn poll_consumer(&self, cx: &mut Context<'_>) -> Poll<()> {
        self.consumer_signal.poll_wait(cx)
    }

    fn poll_producer(&self, cx: &mut Context<'_>) -> Poll<()> {
        self.producer_signal.poll_wait(cx)
    }

    fn notify_producer(&self) {
        self.producer_signal.signal(());
    }

    fn notify_consumer(&self) {
        self.consumer_signal.signal(());
    }

    fn try_producer_busy(&self) -> bool {
        READY_STATE == atomic::swap(&self.producer_state, BUSY_STATE, Ordering::SeqCst)
    }

    fn try_consumer_busy(&self) -> bool {
        READY_STATE == atomic::swap(&self.consumer_state, BUSY_STATE, Ordering::SeqCst)
    }

    fn set_consumer_ready(&self) {
        self.consumer_state.store(READY_STATE, Ordering::SeqCst);
    }

    fn set_producer_ready(&self) {
        self.producer_state.store(READY_STATE, Ordering::SeqCst);
    }
}

pub struct SpscQueue<T, N>
where
    T: 'static,
    N: ArrayLength<T> + 'static,
{
    actor: ActorContext<SpscQueueActor<T, N>>,
    queue: UnsafeCell<HQueue<T, N>>,
    shared: Shared<T, N>,
}

impl<T, N> SpscQueue<T, N>
where
    T: 'static,
    N: ArrayLength<T> + 'static,
{
    pub fn new() -> Self {
        Self {
            actor: ActorContext::new(SpscQueueActor::new()).with_name("spsc_queue_actor"),
            queue: UnsafeCell::new(HQueue::new()),
            shared: Shared::new(),
        }
    }
}

impl<T, N> Package for SpscQueue<T, N>
where
    N: ArrayLength<T> + 'static,
{
    type Primary = SpscQueueActor<T, N>;
    type Configuration = ();
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let (prod, cons) = unsafe { (&mut *self.queue.get()).split() };

        self.shared.set_producer(prod);
        self.shared.set_consumer(cons);
        self.actor.mount(&self.shared, supervisor)
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

pub struct SpscQueueActor<T, N>
where
    T: 'static,
    N: ArrayLength<T> + 'static,
{
    shared: Option<&'static Shared<T, N>>,
}

impl<T, N> SpscQueueActor<T, N>
where
    T: 'static,
    N: ArrayLength<T> + 'static,
{
    pub fn new() -> Self {
        Self { shared: None }
    }
}

impl<T, N> Actor for SpscQueueActor<T, N>
where
    N: ArrayLength<T> + 'static,
{
    type Configuration = &'static Shared<T, N>;
    type Request = QueueRequest<T>;
    type Response = QueueResponse<T>;
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.shared.replace(config);
    }

    fn on_request(self, request: QueueRequest<T>) -> Response<Self> {
        match request {
            QueueRequest::Enqueue(element) => {
                let shared = self.shared.as_ref().unwrap();
                if shared.try_producer_busy() {
                    shared.pending_enqueue.borrow_mut().replace(element);
                    let future = EnqueueFuture::new(shared);
                    Response::immediate_future(self, future)
                } else {
                    Response::immediate(self, Err(Error::ProducerBusy))
                }
            }
            QueueRequest::Dequeue => {
                let shared = self.shared.as_ref().unwrap();
                if shared.try_consumer_busy() {
                    let future = DequeueFuture::new(shared);
                    Response::immediate_future(self, future)
                } else {
                    Response::immediate(self, Err(Error::ConsumerBusy))
                }
            }
        }
    }
}

pub struct EnqueueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    shared: &'static Shared<T, N>,
}

impl<T, N> EnqueueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    pub fn new(shared: &'static Shared<T, N>) -> Self {
        Self { shared }
    }
}

impl<T, N> Future for EnqueueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    type Output = Result<(), Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let producer = self.shared.producer.borrow();
        let mut producer = producer.as_ref().unwrap().borrow_mut();
        loop {
            if producer.ready() {
                producer
                    .enqueue(self.shared.pending_enqueue.borrow_mut().take().unwrap())
                    .ok()
                    .unwrap();
                self.shared.set_producer_ready();
                self.shared.notify_consumer();
                return Poll::Ready(Ok(()));
            } else {
                match self.shared.poll_producer(cx) {
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                    _ => {}
                }
            }
        }
    }
}

pub struct DequeueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    shared: &'static Shared<T, N>,
}

impl<T, N> DequeueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    pub fn new(shared: &'static Shared<T, N>) -> Self {
        Self { shared }
    }
}

impl<T, N> Future for DequeueFuture<T, N>
where
    N: ArrayLength<T> + 'static,
    T: 'static,
{
    type Output = Result<T, Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let consumer = self.shared.consumer.borrow();
        let mut consumer = consumer.as_ref().unwrap().borrow_mut();
        loop {
            match consumer.dequeue() {
                Some(item) => {
                    self.shared.set_consumer_ready();
                    self.shared.notify_producer();
                    return Poll::Ready(Ok(item));
                }
                None => match self.shared.poll_consumer(cx) {
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                    _ => {}
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_actor() {}
}
