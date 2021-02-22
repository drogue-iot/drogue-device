use crate::synchronization::Signal;
use bbqueue::{BBBuffer, ConstBBBuffer, Consumer, Error as BBQueueError, GrantR, GrantW, Producer};

pub use bbqueue::{consts, ArrayLength};
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

struct Inner<'a, N>
where
    N: ArrayLength<u8>,
{
    producer: Option<RefCell<Producer<'a, N>>>,
    consumer: Option<RefCell<Consumer<'a, N>>>,
    producer_signal: Signal<()>,
    consumer_signal: Signal<()>,
}

impl<'a, N> Inner<'a, N>
where
    N: ArrayLength<u8>,
{
    fn new() -> Self {
        Self {
            producer: None,
            consumer: None,
            consumer_signal: Signal::new(),
            producer_signal: Signal::new(),
        }
    }

    fn set_producer(&mut self, producer: Producer<'a, N>) {
        self.producer.replace(RefCell::new(producer));
    }

    fn set_consumer(&mut self, consumer: Consumer<'a, N>) {
        self.consumer.replace(RefCell::new(consumer));
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

    fn prepare_write(&'a self, nbytes: usize) -> Result<AsyncBBProducerGrant<'a, N>, BBQueueError> {
        let mut producer = self.producer.as_ref().unwrap().borrow_mut();
        let grant = producer.grant_max_remaining(nbytes)?;
        Ok(AsyncBBProducerGrant { inner: self, grant })
    }

    fn prepare_read(&'a self) -> Result<AsyncBBConsumerGrant<'a, N>, BBQueueError> {
        let mut consumer = self.consumer.as_ref().unwrap().borrow_mut();
        let grant = consumer.read()?;
        Ok(AsyncBBConsumerGrant { inner: self, grant })
    }
}

pub struct AsyncBBProducerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    inner: &'a Inner<'a, N>,
    grant: GrantW<'a, N>,
}

impl<'a, N> AsyncBBProducerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    pub fn buf(&mut self) -> &mut [u8] {
        self.grant.buf()
    }

    pub fn commit(self, nbytes: usize) {
        self.grant.commit(nbytes);
        if nbytes > 0 {
            self.inner.notify_consumer();
        }
    }
}

pub struct AsyncBBConsumerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    inner: &'a Inner<'a, N>,
    grant: GrantR<'a, N>,
}

impl<'a, N> AsyncBBConsumerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    pub fn buf(&self) -> &[u8] {
        self.grant.buf()
    }

    pub fn len(&self) -> usize {
        self.grant.buf().len()
    }

    pub fn release(self, nbytes: usize) {
        self.grant.release(nbytes);
        if nbytes > 0 {
            self.inner.notify_producer();
        }
    }
}

/// An async wrapper around a BBBuffer (from the bbqueue crate), that allows
/// a producer or consumer to perform DMA-friendly write/reads, while the other
/// end may use async to read/write from/to the buffer.
pub struct AsyncBBBuffer<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    queue: UnsafeCell<BBBuffer<N>>,
    inner: Inner<'a, N>,
}

#[derive(Debug)]
pub enum Error {
    BufferFull,
    BufferEmpty,
    Other,
}

impl<'a, N> AsyncBBBuffer<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    pub fn new() -> Self {
        Self {
            queue: UnsafeCell::new(BBBuffer(ConstBBBuffer::new())),
            inner: Inner::new(),
        }
    }

    pub fn split(&'static mut self) -> (AsyncBBProducer<N>, AsyncBBConsumer<N>) {
        let (prod, cons) = unsafe { (&*self.queue.get()).try_split().unwrap() };
        self.inner.set_producer(prod);
        self.inner.set_consumer(cons);
        (
            AsyncBBProducer::new(&self.inner),
            AsyncBBConsumer::new(&self.inner),
        )
    }
}

pub struct AsyncBBProducer<N>
where
    N: ArrayLength<u8> + 'static,
{
    inner: &'static Inner<'static, N>,
}

impl<N> AsyncBBProducer<N>
where
    N: ArrayLength<u8> + 'static,
{
    fn new(inner: &'static Inner<'static, N>) -> Self {
        Self { inner }
    }

    pub fn prepare_write(&self, nbytes: usize) -> Result<AsyncBBProducerGrant<'static, N>, Error> {
        self.inner.prepare_write(nbytes).map_err(|e| match e {
            BBQueueError::InsufficientSize => Error::BufferFull,
            _ => Error::Other,
        })
    }

    pub unsafe fn write<'a>(&self, write_buf: &'a [u8]) -> AsyncWrite<N> {
        AsyncWrite {
            inner: self.inner,
            bytes_left: write_buf.len(),
            write_buf: core::mem::transmute::<&'a [u8], &'static [u8]>(write_buf),
        }
    }
}

pub struct AsyncWrite<N>
where
    N: ArrayLength<u8> + 'static,
{
    inner: &'static Inner<'static, N>,
    write_buf: &'static [u8],
    bytes_left: usize,
}

impl<N> Future for AsyncWrite<N>
where
    N: ArrayLength<u8> + 'static,
{
    type Output = Result<(), Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.inner.prepare_write(self.bytes_left) {
                Ok(mut grant) => {
                    let buf = grant.buf();

                    let wp = self.write_buf.len() - self.bytes_left;
                    let to_copy = core::cmp::min(self.bytes_left, buf.len());

                    log::trace!("COPYING {} bytes from pos {}", to_copy, wp);
                    buf[..to_copy].copy_from_slice(&self.write_buf[wp..wp + to_copy]);

                    self.bytes_left -= to_copy;
                    grant.commit(to_copy);

                    if self.bytes_left == 0 {
                        return Poll::Ready(Ok(()));
                    } else {
                        match self.inner.poll_producer(cx) {
                            Poll::Pending => {
                                return Poll::Pending;
                            }
                            _ => {}
                        }
                    }
                }
                Err(BBQueueError::InsufficientSize) => match self.inner.poll_producer(cx) {
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                    _ => {}
                },
                Err(e) => return Poll::Ready(Err(Error::Other)),
            }
        }
    }
}

pub struct AsyncBBConsumer<N>
where
    N: ArrayLength<u8> + 'static,
{
    inner: &'static Inner<'static, N>,
}

pub struct AsyncRead<N>
where
    N: ArrayLength<u8> + 'static,
{
    inner: &'static Inner<'static, N>,
    buffer: &'static mut [u8],
    remaining: usize,
    cancelled: bool,
}

impl<N> AsyncRead<N>
where
    N: ArrayLength<u8> + 'static,
{
    unsafe fn new<'a>(inner: &'static Inner<N>, buffer: &'a mut [u8]) -> Self {
        Self {
            inner,
            cancelled: false,
            remaining: buffer.len(),
            buffer: core::mem::transmute::<&'a mut [u8], &'static mut [u8]>(buffer),
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

impl<N> Future for AsyncRead<N>
where
    N: ArrayLength<u8> + 'static,
{
    type Output = Result<usize, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut consumer = self.inner.consumer.as_ref().unwrap().borrow_mut();
        if self.cancelled {
            Poll::Ready(Ok(self.buffer.len() - self.remaining))
        } else {
            loop {
                match consumer.read() {
                    Ok(grant) => {
                        let buf = grant.buf();
                        let rp = self.buffer.len() - self.remaining;
                        let to_copy = core::cmp::min(self.remaining, buf.len());

                        self.buffer[rp..rp + to_copy].copy_from_slice(&buf[..to_copy]);
                        self.remaining -= to_copy;
                        grant.release(to_copy);
                        self.inner.notify_producer();
                        if self.remaining == 0 {
                            return Poll::Ready(Ok(rp + to_copy));
                        } else {
                            match self.inner.poll_consumer(cx) {
                                Poll::Pending => {
                                    return Poll::Pending;
                                }
                                _ => {}
                            }
                        }
                    }
                    // If there was no data available, but we got signaled in the meantime, try again
                    Err(BBQueueError::InsufficientSize) => match self.inner.poll_consumer(cx) {
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                        _ => {}
                    },
                    Err(e) => return Poll::Ready(Err(Error::Other)),
                }
            }
        }
    }
}

impl<N> AsyncBBConsumer<N>
where
    N: ArrayLength<u8> + 'static,
{
    fn new(inner: &'static Inner<'static, N>) -> Self {
        Self { inner }
    }

    pub fn prepare_read(&self) -> Result<AsyncBBConsumerGrant<'static, N>, Error> {
        self.inner.prepare_read().map_err(|e| match e {
            BBQueueError::InsufficientSize => Error::BufferEmpty,
            _ => Error::Other,
        })
    }

    /// Read from the consumer into the provided buffer. The returned future
    /// will complete once all bytes have been read.
    ///
    /// Safety: the returned future must be awaited before the provided buffer is
    /// dropped or reused.
    pub unsafe fn read<'a>(&self, buffer: &'a mut [u8]) -> AsyncRead<N> {
        AsyncRead::new(self.inner, buffer)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use futures::executor::block_on;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    fn split<N>(buffer: &mut AsyncBBBuffer<N>) -> (AsyncBBProducer<N>, AsyncBBConsumer<N>)
    where
        N: ArrayLength<u8>,
    {
        let buffer = unsafe {
            core::mem::transmute::<&mut AsyncBBBuffer<N>, &'static mut AsyncBBBuffer<N>>(buffer)
        };
        buffer.split()
    }

    #[test]
    fn test_queue() {
        setup();
        let mut queue: AsyncBBBuffer<consts::U8> = AsyncBBBuffer::new();
        let (prod, cons) = split(&mut queue);

        unsafe {
            let mut rx_buf = [0; 4];

            let rx_future = cons.read(&mut rx_buf);

            block_on(prod.write(r"helo".as_bytes())).unwrap();

            let result = block_on(rx_future).unwrap();
            assert_eq!(4, result);
            assert_eq!(b"helo", &rx_buf);
        }
    }

    #[test]
    fn test_interrupt_queue() {
        setup();

        let mut queue: AsyncBBBuffer<consts::U128> = AsyncBBBuffer::new();
        let (prod, cons) = split(&mut queue);

        {
            let mut wgrant = prod.prepare_write(128).unwrap();
            let buf = wgrant.buf();
            for i in 0..62 {
                buf[i] = i as u8;
            }

            wgrant.commit(62);
        }

        for i in 0..50 {
            let mut rx_buf = [0; 1];
            block_on(unsafe { cons.read(&mut rx_buf) }).unwrap();
        }
    }
}
