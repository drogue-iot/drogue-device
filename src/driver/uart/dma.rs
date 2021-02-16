use crate::prelude::*;

use crate::domain::time::duration::{Duration, Milliseconds};
pub use crate::api::uart::Error;
use crate::api::{
    scheduler::*,
    uart::{Uart, UartRx, UartRxTimeout, UartTx},
};
use crate::hal::uart::dma::DmaUartHal;
use crate::interrupt::{Interrupt, InterruptContext};
use crate::package::Package;
use crate::synchronization::Signal;

use core::cell::{Cell, RefCell};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::Nr;

pub struct UartActor<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    me: Option<Address<Self>>,
    shared: Option<&'static Shared<U, T>>,
}

pub struct UartInterrupt<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    shared: Option<&'static Shared<U, T>>,
}

pub struct Shared<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    uart: U,
    timer: RefCell<Option<Address<T>>>,
    tx_state: Cell<State>,
    rx_state: Cell<State>,
    tx_done: Signal<Result<(), Error>>,
    rx_done: Signal<Result<usize, Error>>,
}

pub struct DmaUart<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    actor: ActorContext<UartActor<U, T>>,
    interrupt: InterruptContext<UartInterrupt<U, T>>,
    shared: Shared<U, T>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum State {
    Ready,
    InProgress,
}

impl<U, T> Shared<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    fn new(uart: U) -> Self {
        Self {
            tx_done: Signal::new(),
            rx_done: Signal::new(),
            uart,
            timer: RefCell::new(None),
            tx_state: Cell::new(State::Ready),
            rx_state: Cell::new(State::Ready),
        }
    }
}

impl<U, T> DmaUart<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    pub fn new<IRQ>(uart: U, irq: IRQ) -> Self
    where
        IRQ: Nr,
    {
        Self {
            actor: ActorContext::new(UartActor::new()).with_name("uart_actor"),
            interrupt: InterruptContext::new(UartInterrupt::new(), irq).with_name("uart_interrupt"),
            shared: Shared::new(uart),
        }
    }
}

impl<U, T> Package for DmaUart<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    type Primary = UartActor<U, T>;
    type Configuration = Address<T>;
    fn mount(
        &'static self,
        timer: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<UartActor<U, T>> {
        self.shared.timer.borrow_mut().replace(timer);
        let addr = self.actor.mount(&self.shared, supervisor);
        self.interrupt.mount(&self.shared, supervisor);

        addr
    }
}

impl<U, T> UartActor<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    pub fn new() -> Self {
        Self {
            shared: None,
            me: None,
        }
    }
}

// DMA implementation of the trait
impl<U, T> Uart for UartActor<U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    /// Receive bytes into the provided rx_buffer. The memory pointed to by the buffer must be available until the return future is await'ed
    fn read<'a>(self, message: UartRx<'a>) -> Response<Self, Result<usize, Error>> {
        let shared = self.shared.as_ref().unwrap();
        match shared.rx_state.get() {
            State::Ready => {
                log::trace!("NO RX in progress");
                shared.rx_done.reset();
                shared.rx_state.set(State::InProgress);
                match shared.uart.start_read(message.0) {
                    Ok(_) => {
                        log::trace!("Starting RX");
                        let future = RxFuture::new(shared);
                        Response::immediate_future(self, future)
                    }
                    Err(e) => Response::immediate(self, Err(e)),
                }
            }
            _ => Response::immediate(self, Err(Error::RxInProgress)),
        }
    }

    /// Receive bytes into the provided rx_buffer. The memory pointed to by the buffer must be available until the return future is await'ed
    fn read_with_timeout<'a, DUR>(
        self,
        message: UartRxTimeout<'a, DUR>,
    ) -> Response<Self, Result<usize, Error>>
    where
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        let shared = self.shared.as_ref().unwrap();
        match shared.rx_state.get() {
            State::Ready => {
                log::trace!("NO RX in progress");
                shared.rx_done.reset();
                shared.rx_state.set(State::InProgress);
                match shared.uart.start_read(message.0) {
                    Ok(_) => {
                        log::trace!("Starting RX");
                        // Start the timer
                        shared.timer.borrow().as_ref().unwrap().schedule(
                            message.1,
                            RxTimeout,
                            self.me.as_ref().unwrap().clone(),
                        );
                        let future = RxFuture::new(shared);
                        Response::immediate_future(self, future)
                    }
                    Err(e) => Response::immediate(self, Err(e)),
                }
            }
            _ => Response::immediate(self, Err(Error::RxInProgress)),
        }
    }

    /// Transmit bytes from provided tx_buffer over UART. The memory pointed to by the buffer must be available until the return future is await'ed
    fn write<'a>(self, message: UartTx<'a>) -> Response<Self, Result<(), Error>> {
        let shared = self.shared.as_ref().unwrap();
        match shared.tx_state.get() {
            State::Ready => {
                log::trace!("NO TX in progress");
                shared.tx_done.reset();
                shared.tx_state.set(State::InProgress);
                match shared.uart.start_write(message.0) {
                    Ok(_) => {
                        log::trace!("Starting TX");
                        let future = TxFuture::new(shared);
                        Response::immediate_future(self, future)
                    }
                    Err(e) => Response::immediate(self, Err(e)),
                }
            }
            _ => Response::immediate(self, Err(Error::TxInProgress)),
        }
    }
}

impl<U, T> NotifyHandler<RxTimeout> for UartActor<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    fn on_notify(self, message: RxTimeout) -> Completion<Self> {
        let shared = self.shared.as_ref().unwrap();
        if State::InProgress == shared.rx_state.get() {
            shared.uart.cancel_read();
        }
        Completion::immediate(self)
    }
}

impl<U, T> Actor for UartActor<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    type Configuration = &'static Shared<U, T>;

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.me.replace(me);
        self.shared.replace(config);
    }
}

impl<U, T> UartInterrupt<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    pub fn new() -> Self {
        Self { shared: None }
    }
}

impl<U, T> Actor for UartInterrupt<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    type Configuration = &'static Shared<U, T>;

    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.shared.replace(config);
    }
}

impl<U, T> Interrupt for UartInterrupt<U, T>
where
    U: DmaUartHal,
    T: Scheduler + 'static,
{
    fn on_interrupt(&mut self) {
        let shared = self.shared.as_ref().unwrap();
        let (tx_done, rx_done) = shared.uart.process_interrupts();
        log::trace!(
            "[UART ISR] TX SIGNALED: {}. RX SIGNALED: {}. TX DONE: {}. RX DONE: {}",
            shared.tx_done.signaled(),
            shared.rx_done.signaled(),
            tx_done,
            rx_done,
        );

        if tx_done {
            shared.tx_done.signal(shared.uart.finish_write());
        }

        if rx_done {
            shared.rx_done.signal(shared.uart.finish_read());
        }
    }
}

pub struct TxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    shared: &'a Shared<U, T>,
}

impl<'a, U, T> TxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    fn new(shared: &'a Shared<U, T>) -> Self {
        Self { shared }
    }
}

impl<'a, U, T> Future for TxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    type Output = Result<(), Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("Polling TX: {:?}", self.shared.tx_state.get());
        if State::InProgress == self.shared.tx_state.get() {
            if let Poll::Ready(result) = self.shared.tx_done.poll_wait(cx) {
                self.shared.tx_state.set(State::Ready);
                log::trace!("Marked TX future complete. Set ready");
                return Poll::Ready(result);
            }
        }
        return Poll::Pending;
    }
}

impl<'a, U, T> Drop for TxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    fn drop(&mut self) {
        if State::InProgress == self.shared.tx_state.get() {
            self.shared.uart.cancel_write();
        }
    }
}

pub struct RxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    shared: &'a Shared<U, T>,
}

impl<'a, U, T> RxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    fn new(shared: &'a Shared<U, T>) -> Self {
        Self { shared }
    }
}

impl<'a, U, T> Future for RxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    type Output = Result<usize, Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("Polling RX: {:?}", self.shared.rx_state.get());
        if State::InProgress == self.shared.rx_state.get() {
            if let Poll::Ready(result) = self.shared.rx_done.poll_wait(cx) {
                self.shared.rx_state.set(State::Ready);
                log::trace!("Marked RX future complete. Set ready");
                return Poll::Ready(result);
            }
        }
        return Poll::Pending;
    }
}

impl<'a, U, T> Drop for RxFuture<'a, U, T>
where
    U: DmaUartHal + 'static,
    T: Scheduler + 'static,
{
    fn drop(&mut self) {
        if State::InProgress == self.shared.rx_state.get() {
            self.shared.uart.cancel_read();
        }
    }
}

#[derive(Clone)]
struct RxTimeout;
