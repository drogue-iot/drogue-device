use crate::api::uart::Error;
use crate::platform::atomic;
use crate::synchronization::Signal;
use crate::util::dma::async_bbqueue::*;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use core::task::{Context, Poll};

const READY_STATE: bool = false;
const BUSY_STATE: bool = true;

pub struct ActorState {
    tx_state: AtomicBool,
    rx_state: AtomicBool,
    rx_timeout: Signal<()>,
}

impl ActorState {
    pub fn new() -> Self {
        Self {
            tx_state: AtomicBool::new(READY_STATE),
            rx_timeout: Signal::new(),
            rx_state: AtomicBool::new(READY_STATE),
        }
    }

    pub fn try_rx_busy(&self) -> bool {
        READY_STATE == atomic::swap(&self.rx_state, BUSY_STATE, Ordering::SeqCst)
    }

    pub fn try_tx_busy(&self) -> bool {
        READY_STATE == atomic::swap(&self.tx_state, BUSY_STATE, Ordering::SeqCst)
    }

    pub fn reset_rx_timeout(&self) {
        self.rx_timeout.reset();
    }

    pub fn signal_rx_timeout(&self) {
        self.rx_timeout.signal(());
    }

    fn set_rx_ready(&self) {
        self.rx_state.store(READY_STATE, Ordering::SeqCst);
    }

    fn set_tx_ready(&self) {
        self.tx_state.store(READY_STATE, Ordering::SeqCst);
    }

    fn poll_rx_timeout(&self, cx: &mut Context<'_>) -> Poll<()> {
        self.rx_timeout.poll_wait(cx)
    }
}

pub struct TxFuture<'a, TXN>
where
    TXN: ArrayLength<u8> + 'static,
{
    future: AsyncWrite<TXN>,
    shared: &'a ActorState,
}

impl<'a, TXN> TxFuture<'a, TXN>
where
    TXN: ArrayLength<u8> + 'static,
{
    pub fn new(future: AsyncWrite<TXN>, shared: &'a ActorState) -> Self {
        Self { future, shared }
    }
}

impl<'a, TXN> Future for TxFuture<'a, TXN>
where
    TXN: ArrayLength<u8> + 'static,
{
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Future::poll(Pin::new(&mut self.future), cx) {
            Poll::Ready(result) => {
                self.shared.set_tx_ready();
                Poll::Ready(result.map_err(|_| Error::Receive))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct RxFuture<'a, RXN>
where
    RXN: ArrayLength<u8> + 'static,
{
    future: AsyncRead<RXN>,
    shared: &'a ActorState,
}

impl<'a, RXN> RxFuture<'a, RXN>
where
    RXN: ArrayLength<u8> + 'static,
{
    pub fn new(future: AsyncRead<RXN>, shared: &'a ActorState) -> Self {
        Self { future, shared }
    }
}

impl<'a, RXN> Future for RxFuture<'a, RXN>
where
    RXN: ArrayLength<u8> + 'static,
{
    type Output = Result<usize, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(_) = self.shared.poll_rx_timeout(cx) {
            self.future.cancel();
        }

        match Future::poll(Pin::new(&mut self.future), cx) {
            Poll::Ready(result) => {
                self.shared.set_rx_ready();
                return Poll::Ready(result.map_err(|_| Error::Receive));
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
