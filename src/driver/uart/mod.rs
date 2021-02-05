use crate::actor::{Actor, ActorContext, Configurable};
use crate::address::Address;
use crate::bus::EventBus;
use crate::device::Device;
use crate::prelude::*;

pub use crate::hal::uart::Error;
use crate::hal::uart::Uart as HalUart;
use crate::interrupt::{Interrupt, InterruptContext};
use crate::package::Package;
use crate::synchronization::{Mutex, Signal};

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::Nr;

pub struct Shared<U>
where
    U: HalUart + 'static,
{
    uart: U,
    tx_done: Signal<Result<(), Error>>,
    rx_done: Signal<Result<usize, Error>>,
}

pub struct Uart<U>
where
    U: HalUart + 'static,
{
    peripheral: ActorContext<Mutex<UartPeripheral<U>>>,
    irq: InterruptContext<UartInterrupt<U>>,
    shared: Shared<U>,
}

#[derive(Clone, PartialEq)]
pub enum State {
    Ready,
    InProgress,
}

impl<U> Uart<U>
where
    U: HalUart + 'static,
{
    pub fn new<IRQ>(uart: U, irq: IRQ) -> Self
    where
        IRQ: Nr,
    {
        Self {
            peripheral: ActorContext::new(Mutex::new(UartPeripheral::new())),
            irq: InterruptContext::new(UartInterrupt::new(), irq),
            shared: Shared {
                uart,
                tx_done: Signal::new(),
                rx_done: Signal::new(),
            },
        }
    }
}

impl<D, U> Package<D, Mutex<UartPeripheral<U>>> for Uart<U>
where
    D: Device,
    U: HalUart,
{
    fn mount(
        &'static self,
        _: Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<Mutex<UartPeripheral<U>>> {
        let peripheral = self.peripheral.mount(supervisor);
        let irq = self.irq.mount(supervisor);

        //irq.bind(&peripheral.clone());
        //peripheral.notify(&self.uart);
        //irq.notify(&self.uart);
        self.peripheral.configure(&self.shared);
        self.irq.configure(&self.shared);

        peripheral
    }
}

pub struct UartPeripheral<U>
where
    U: HalUart + 'static,
{
    tx_state: State,
    rx_state: State,

    uart: Option<&'static U>,

    tx_done: Option<&'static Signal<Result<(), Error>>>,
    rx_done: Option<&'static Signal<Result<usize, Error>>>,
}

impl<U> UartPeripheral<U>
where
    U: HalUart,
{
    pub fn new() -> Self {
        Self {
            tx_done: None,
            rx_done: None,
            uart: None,
            tx_state: State::Ready,
            rx_state: State::Ready,
        }
    }

    /// Receive bytes into the provided rx_buffer. The memory pointed to by the buffer must be available until the return future is await'ed
    pub fn read<'a>(&'a mut self, rx_buffer: &mut [u8]) -> RxFuture<'a, U> {
        match self.rx_state {
            State::Ready => {
                log::trace!("NO RX in progress");
                self.rx_done.unwrap().reset();
                self.rx_state = State::InProgress;
                let uart = self.uart.unwrap();
                match uart.start_read(rx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting RX");
                        RxFuture::Defer(self)
                    }
                    Err(e) => RxFuture::Error(e),
                }
            }
            _ => RxFuture::Error(Error::RxInProgress),
        }
    }

    /// Transmit bytes from provided tx_buffer over UART. The memory pointed to by the buffer must be available until the return future is await'ed
    pub fn write<'a>(&'a mut self, tx_buffer: &[u8]) -> TxFuture<'a, U> {
        match self.tx_state {
            State::Ready => {
                log::trace!("NO TX in progress");
                self.tx_done.unwrap().reset();
                self.tx_state = State::InProgress;
                let uart = self.uart.unwrap();
                match uart.start_write(tx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting TX");
                        TxFuture::Defer(self)
                    }
                    Err(e) => TxFuture::Error(e),
                }
            }
            _ => TxFuture::Error(Error::TxInProgress),
        }
    }
}

impl<U> Configurable for UartPeripheral<U>
where
    U: HalUart + 'static,
{
    type Configuration = Shared<U>;

    fn configure(&mut self, config: &'static Self::Configuration) {
        self.uart.replace(&config.uart);
        self.tx_done.replace(&config.tx_done);
        self.rx_done.replace(&config.rx_done);
    }
}

pub struct UartInterrupt<U>
where
    U: HalUart + 'static,
{
    uart: Option<&'static U>,
    tx_done: Option<&'static Signal<Result<(), Error>>>,
    rx_done: Option<&'static Signal<Result<usize, Error>>>,
}

impl<U> UartInterrupt<U>
where
    U: HalUart,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            tx_done: None,
            rx_done: None,
        }
    }
}

impl<U> Configurable for UartInterrupt<U>
    where
        U: HalUart + 'static,
{
    type Configuration = Shared<U>;

    fn configure(&mut self, config: &'static Self::Configuration) {
        self.uart.replace(&config.uart);
        self.tx_done.replace(&config.tx_done);
        self.rx_done.replace(&config.rx_done);
    }
}

impl<U> Actor for UartInterrupt<U> where U: HalUart {}

impl<U> Interrupt for UartInterrupt<U>
where
    U: HalUart,
{
    fn on_interrupt(&mut self) {
        let uart = self.uart.unwrap();
        let (tx_done, rx_done) = uart.process_interrupts();
        log::trace!(
            "[UART ISR] TX WAKER: {}. RX WAKER: {}. TX DONE: {}. RX DONE: {}",
            self.tx_done.as_ref().unwrap().signaled(),
            self.rx_done.as_ref().unwrap().signaled(),
            tx_done,
            rx_done,
        );

        if tx_done {
            self.tx_done.as_ref().unwrap().signal(uart.finish_write());
        }

        if rx_done {
            self.rx_done.as_ref().unwrap().signal(uart.finish_read());
        }
    }
}

impl<U> Actor for UartPeripheral<U> where U: HalUart {}

pub enum TxFuture<'a, U>
where
    U: HalUart + 'static,
{
    Defer(&'a mut UartPeripheral<U>),
    Error(Error),
}

impl<'a, U> Future for TxFuture<'a, U>
where
    U: HalUart + 'static,
{
    type Output = Result<(), Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut *self {
            TxFuture::Defer(ref mut p) => {
                if State::InProgress == p.tx_state {
                    let done = p.tx_done.unwrap();
                    if let Poll::Ready(result) = done.poll_wait(cx) {
                        p.tx_state = State::Ready;
                        log::trace!("Marking future complete");
                        return Poll::Ready(result);
                    }
                }
                return Poll::Pending;
            }
            TxFuture::Error(err) => return Poll::Ready(Err(err.clone())),
        }
    }
}

impl<'a, U> Drop for TxFuture<'a, U>
where
    U: HalUart + 'static,
{
    fn drop(&mut self) {
        match self {
            TxFuture::Defer(ref mut p) => {
                if State::InProgress == p.tx_state {
                    p.uart.unwrap().cancel_write();
                }
            }
            _ => {}
        }
    }
}

pub enum RxFuture<'a, U>
where
    U: HalUart + 'static,
{
    Defer(&'a mut UartPeripheral<U>),
    Error(Error),
}

impl<'a, U> Future for RxFuture<'a, U>
where
    U: HalUart + 'static,
{
    type Output = Result<usize, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut *self {
            RxFuture::Defer(ref mut p) => {
                if State::InProgress == p.rx_state {
                    let done = p.rx_done.unwrap();
                    if let Poll::Ready(result) = done.poll_wait(cx) {
                        p.rx_state = State::Ready;
                        log::trace!("Marking future complete");
                        return Poll::Ready(result);
                    }
                }
                return Poll::Pending;
            }
            RxFuture::Error(err) => return Poll::Ready(Err(err.clone())),
        }
    }
}

impl<'a, U> Drop for RxFuture<'a, U>
where
    U: HalUart + 'static,
{
    fn drop(&mut self) {
        match self {
            RxFuture::Defer(ref mut p) => {
                if State::InProgress == p.rx_state {
                    p.uart.unwrap().cancel_read();
                }
            }
            _ => {}
        }
    }
}
