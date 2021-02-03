use crate::actor::{Actor, ActorContext};
use crate::address::Address;
use crate::bind::Bind;
use crate::bus::EventBus;
use crate::device::Device;
use crate::prelude::*;

pub use crate::hal::uart::Error;
use crate::hal::uart::Uart as HalUart;
use crate::handler::{Completion, NotifyHandler};
use crate::interrupt::{Interrupt, InterruptContext};
use crate::package::Package;
use crate::synchronization::{Mutex, Signal};

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::Nr;

pub struct Uart<U>
where
    U: HalUart + 'static,
{
    uart: U,
    peripheral: ActorContext<Mutex<UartPeripheral<U>>>,
    irq: InterruptContext<UartInterrupt<U>>,
}

#[derive(Clone)]
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
            uart,
            peripheral: ActorContext::new(Mutex::new(UartPeripheral::new())),
            irq: InterruptContext::new(UartInterrupt::new(), irq),
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
        _: &Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<Mutex<UartPeripheral<U>>> {
        let peripheral = self.peripheral.mount(supervisor);
        let irq = self.irq.mount(supervisor);

        irq.bind(&peripheral.clone());
        peripheral.notify(&self.uart);
        irq.notify(&self.uart);

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

    pub fn read<'a>(&'a mut self, rx_buffer: &mut [u8]) -> UartFuture<'a, usize> {
        match self.rx_state {
            State::Ready => {
                log::trace!("NO RX in progress");
                self.rx_done.unwrap().reset();
                self.rx_state = State::InProgress;
                let uart = self.uart.unwrap();
                match uart.start_read(rx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting RX");
                        UartFuture::Defer(&mut self.rx_state, self.rx_done)
                    }
                    Err(e) => UartFuture::Error(e),
                }
            }
            _ => UartFuture::Error(Error::RxInProgress),
        }
    }

    pub fn write<'a>(&'a mut self, tx_buffer: &[u8]) -> UartFuture<'a, ()> {
        match self.tx_state {
            State::Ready => {
                log::trace!("NO TX in progress");
                self.tx_done.unwrap().reset();
                self.tx_state = State::InProgress;
                let uart = self.uart.unwrap();
                match uart.start_write(tx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting TX");
                        UartFuture::Defer(&mut self.tx_state, self.tx_done)
                    }
                    Err(e) => UartFuture::Error(e),
                }
            }
            _ => UartFuture::Error(Error::TxInProgress),
        }
    }
}

pub struct UartInterrupt<U>
where
    U: HalUart + 'static,
{
    uart: Option<&'static U>,
    tx_done: Signal<Result<(), Error>>,
    rx_done: Signal<Result<usize, Error>>,
}

impl<U> UartInterrupt<U>
where
    U: HalUart,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            tx_done: Signal::new(),
            rx_done: Signal::new(),
        }
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
            self.tx_done.signaled(),
            self.rx_done.signaled(),
            tx_done,
            rx_done,
        );

        if tx_done {
            self.tx_done.signal(uart.finish_write());
        }

        if rx_done {
            self.rx_done.signal(uart.finish_read());
        }
    }
}

impl<U> NotifyHandler<&'static U> for Mutex<UartPeripheral<U>>
where
    U: HalUart + 'static,
{
    fn on_notify(&'static mut self, uart: &'static U) -> Completion {
        self.val.as_mut().unwrap().uart.replace(uart);
        Completion::immediate()
    }
}

impl<U>
    NotifyHandler<(
        &'static Signal<Result<(), Error>>,
        &'static Signal<Result<usize, Error>>,
    )> for Mutex<UartPeripheral<U>>
where
    U: HalUart,
{
    fn on_notify(
        &'static mut self,
        signals: (
            &'static Signal<Result<(), Error>>,
            &'static Signal<Result<usize, Error>>,
        ),
    ) -> Completion {
        self.val.as_mut().unwrap().tx_done.replace(signals.0);
        self.val.as_mut().unwrap().rx_done.replace(signals.1);
        Completion::immediate()
    }
}

impl<U> Bind<Mutex<UartPeripheral<U>>> for UartInterrupt<U>
where
    U: HalUart,
{
    fn on_bind(&'static mut self, address: Address<Mutex<UartPeripheral<U>>>) {
        address.notify((&self.tx_done, &self.rx_done));
    }
}

impl<U> NotifyHandler<&'static U> for UartInterrupt<U>
where
    U: HalUart + 'static,
{
    fn on_notify(&'static mut self, uart: &'static U) -> Completion {
        self.uart.replace(uart);
        Completion::immediate()
    }
}

impl<U> Actor for UartPeripheral<U> where U: HalUart {}

pub enum UartFuture<'a, R>
where
    R: 'static,
{
    Defer(&'a mut State, Option<&'static Signal<Result<R, Error>>>),
    Error(Error),
}

impl<'a, R> Future for UartFuture<'a, R> {
    type Output = Result<R, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut *self {
            UartFuture::Defer(ref mut state, ref done) => {
                if let State::InProgress = state {
                    let done = done.unwrap();
                    if let Poll::Ready(result) = done.poll_wait(cx) {
                        **state = State::Ready;
                        log::trace!("Marking future complete");
                        return Poll::Ready(result);
                    }
                }
                return Poll::Pending;
            }
            UartFuture::Error(err) => return Poll::Ready(Err(err.clone())),
        }
    }
}
