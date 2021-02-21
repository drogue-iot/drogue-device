use crate::synchronization::Signal;
use bbqueue::{
    consts, ArrayLength, BBBuffer, ConstBBBuffer, Consumer, Error as BBQueueError, GrantW, Producer,
};
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

struct QueueInner<'a, N>
where
    N: ArrayLength<u8>,
{
    producer: Option<RefCell<Producer<'a, N>>>,
    consumer: Option<RefCell<Consumer<'a, N>>>,
    producer_signal: Signal<()>,
    consumer_signal: Signal<()>,
}

impl<'a, N> QueueInner<'a, N>
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
}

pub struct AsyncBBProducerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    inner: &'a QueueInner<'a, N>,
    grant: GrantW<'a, N>,
}

impl<'a, N> AsyncBBProducerGrant<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    pub fn buf(&mut self) -> &mut [u8] {
        self.grant.buf()
    }

    pub fn commit(mut self, nbytes: usize) {
        self.grant.commit(nbytes);
        if nbytes > 0 {
            self.inner.notify_consumer();
        }
    }
}

pub struct AsyncBBBuffer<'a, N>
where
    N: ArrayLength<u8> + 'a,
{
    queue: UnsafeCell<BBBuffer<N>>,
    inner: QueueInner<'a, N>,
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
            inner: QueueInner::new(),
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
    inner: &'static QueueInner<'static, N>,
}

impl<N> AsyncBBProducer<N>
where
    N: ArrayLength<u8> + 'static,
{
    fn new(inner: &'static QueueInner<'static, N>) -> Self {
        Self { inner }
    }

    pub fn prepare_write(&self, nbytes: usize) -> Result<AsyncBBProducerGrant<'static, N>, Error> {
        self.inner.prepare_write(nbytes).map_err(|e| Error::Other)
    }

    pub fn write<'a>(&self, write_buf: &'a [u8]) -> impl Future<Output = Result<(), Error>> {
        struct WriteFuture<N>
        where
            N: ArrayLength<u8> + 'static,
        {
            inner: &'static QueueInner<'static, N>,
            write_buf: &'static [u8],
            bytes_left: usize,
        }

        impl<N> Future for WriteFuture<N>
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

                            //log::info!("COPYING {} bytes from pos {}", to_copy, wp);
                            buf[..to_copy].copy_from_slice(&self.write_buf[wp..to_copy]);

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

        WriteFuture {
            inner: self.inner,
            bytes_left: write_buf.len(),
            write_buf: unsafe { core::mem::transmute::<&'a [u8], &'static [u8]>(write_buf) },
        }
    }
}

pub struct AsyncBBConsumer<N>
where
    N: ArrayLength<u8> + 'static,
{
    inner: &'static QueueInner<'static, N>,
}

impl<N> AsyncBBConsumer<N>
where
    N: ArrayLength<u8> + 'static,
{
    fn new(inner: &'static QueueInner<'static, N>) -> Self {
        Self { inner }
    }

    pub fn read<'a>(&self, read_buf: &'a mut [u8]) -> impl Future<Output = Result<usize, Error>> {
        struct ReadFuture<N>
        where
            N: ArrayLength<u8> + 'static,
        {
            inner: &'static QueueInner<'static, N>,
            read_buf: &'static mut [u8],
            bytes_left: usize,
        }

        impl<N> Future for ReadFuture<N>
        where
            N: ArrayLength<u8> + 'static,
        {
            type Output = Result<usize, Error>;
            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut consumer = self.inner.consumer.as_ref().unwrap().borrow_mut();
                loop {
                    match consumer.read() {
                        Ok(grant) => {
                            let buf = grant.buf();
                            let rp = self.read_buf.len() - self.bytes_left;
                            let to_copy = core::cmp::min(self.bytes_left, buf.len());

                            self.read_buf[rp..].copy_from_slice(&buf[..to_copy]);
                            self.bytes_left -= to_copy;
                            grant.release(to_copy);
                            self.inner.notify_producer();
                            if self.bytes_left == 0 {
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

        ReadFuture {
            inner: self.inner,
            bytes_left: read_buf.len(),
            read_buf: unsafe { core::mem::transmute::<&'a mut [u8], &'static mut [u8]>(read_buf) },
        }
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

    #[test]
    fn test_queue() {
        setup();
        let mut queue: AsyncBBBuffer<consts::U8> = AsyncBBBuffer::new();
        let mut queue = unsafe {
            core::mem::transmute::<&AsyncBBBuffer<consts::U8>, &'static AsyncBBBuffer<consts::U8>>(
                &queue,
            )
        };
        let (mut prod, cons) = queue.split();

        {
            let mut rx_buf = [0; 4];

            let rx_future = cons.read(&mut rx_buf);

            block_on(prod.write(r"helo".as_bytes()));

            let result = block_on(rx_future).unwrap();
            assert_eq!(4, result);
            assert_eq!(b"helo", &rx_buf);
        }
    }

    #[test]
    fn test_interrupt_queue() {
        setup();

        let mut queue: AsyncBBBuffer<consts::U128> = AsyncBBBuffer::new();
        let mut queue = unsafe {
            core::mem::transmute::<&AsyncBBBuffer<consts::U128>, &'static AsyncBBBuffer<consts::U128>>(
                &queue,
            )
        };
        let (mut prod, cons) = queue.split();

        let mut prod = unsafe {
            core::mem::transmute::<
                &AsyncBBProducer<consts::U128>,
                &'static AsyncBBProducer<consts::U128>,
            >(&mut prod)
        };
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
            block_on(cons.read(&mut rx_buf));
        }
    }
}
