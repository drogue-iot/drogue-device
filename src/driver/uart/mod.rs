use crate::actor::Actor;
use crate::address::Address;
use crate::hal::uart::Uart as HalUart;
use crate::handler::{RequestHandler, Response};
use crate::interrupt::Interrupt;

use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

pub use crate::hal::uart::Error;

pub struct Uart<T>
where
    T: HalUart,
{
    uart: T,
    tx_state: TxState,
    tx_waker: Option<Waker>,
}

#[derive(Clone)]
enum TxState {
    Ready,
    InProgress,
    Done(Result<(), Error>),
}

impl<T> Uart<T>
where
    T: HalUart,
{
    pub fn new(uart: T) -> Self {
        Self {
            uart,
            tx_state: TxState::Ready,
            tx_waker: None,
        }
    }

    fn set_tx_waker(&mut self, waker: Waker) {
        self.tx_waker.replace(waker);
    }

    fn complete_tx(&mut self) -> Option<Result<(), Error>> {
        match &self.tx_state {
            TxState::Done(result) => {
                let result = result.clone();
                self.tx_state = TxState::Ready;
                Some(result)
            }
            _ => None,
        }
    }
}

impl<T> Actor for Uart<T> where T: HalUart {}

pub struct UartTx(pub &'static [u8]);

impl<T> RequestHandler<UartTx> for Uart<T>
where
    T: HalUart,
{
    type Response = Result<(), Error>;

    fn on_request(&'static mut self, message: UartTx) -> Response<Self::Response> {
        match self.tx_state {
            TxState::Ready => {
                log::trace!("NO TX in progress");
                match self.uart.write_start(message.0) {
                    Ok(_) => {
                        self.tx_state = TxState::InProgress;
                        let f = TxFuture::new(self, message);
                        Response::immediate_future(f)
                    }
                    result => return Response::immediate(result),
                }
            }
            _ => Response::immediate(Err(Error::TxInProgress)),
        }
    }
}

impl<T> Interrupt for Uart<T>
where
    T: HalUart,
{
    fn on_interrupt(&mut self) {
        log::trace!("Uart interrupt");
        if let TxState::InProgress = self.tx_state {
            if self.uart.write_done() {
                log::trace!("Marking TX complete");
                self.tx_state = TxState::Done(self.uart.write_finish());
                if let Some(waker) = &self.tx_waker {
                    self.tx_waker.take().unwrap().wake();
                }
            }
        }
    }
}

struct TxFuture<T>
where
    T: HalUart,
{
    uart: UnsafeCell<*mut Uart<T>>,
    data: UartTx,
}

impl<T> TxFuture<T>
where
    T: HalUart,
{
    fn new(uart: &mut Uart<T>, data: UartTx) -> Self {
        Self {
            uart: UnsafeCell::new(uart),
            data,
        }
    }

    fn check_result(&mut self) -> Option<Result<(), Error>> {
        unsafe {
            // make sure we don't run concurrently as uart interrupt
            cortex_m::interrupt::free(|cs| (&mut **self.uart.get()).complete_tx())
        }
    }

    fn set_waker(&mut self, waker: &Waker) {
        unsafe {
            (&mut **self.uart.get()).set_tx_waker(waker.clone());
        }
    }
}

impl<T> Future for TxFuture<T>
where
    T: HalUart,
{
    type Output = Result<(), Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.check_result() {
            Poll::Ready(result)
        } else {
            self.set_waker(cx.waker());
            Poll::Pending
        }
    }
}

impl<T> Address<Uart<T>>
where
    T: HalUart + 'static,
{
    pub async fn write(&self, data: &'static [u8]) -> Result<(), Error> {
        self.request(UartTx(data)).await
    }
}
