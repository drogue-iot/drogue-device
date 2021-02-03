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

use core::cell::UnsafeCell;
use core::future::Future;
use core::ops::Deref;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;

pub struct Uart<U>
where
    U: HalUart + 'static,
{
    uart: U,
    ctx: UartContext<U>,
    peripheral: ActorContext<Mutex<UartPeripheral<U>>>,
    irq: InterruptContext<UartInterrupt<U>>,
}

#[derive(Clone)]
enum TxState {
    Ready,
    InProgress,
}

#[derive(Clone)]
enum RxState {
    Ready,
    InProgress,
}

impl<U> Uart<U>
where
    U: HalUart,
{
    pub fn new<IRQ>(uart: U, irq: IRQ) -> Self
    where
        IRQ: Nr,
    {
        let ctx = UartContext::new(&uart);
        Self {
            uart,
            ctx: ctx.clone(),
            peripheral: ActorContext::new(Mutex::new(UartPeripheral::new(ctx.clone()))),
            irq: InterruptContext::new(UartInterrupt::new(ctx.clone()), irq),
        }
    }
}

pub struct UartContext<U>
where
    U: HalUart,
{
    uart: UnsafeCell<*const U>,
}

impl<U> UartContext<U>
where
    U: HalUart,
{
    fn new(uart: &U) -> Self {
        Self {
            uart: UnsafeCell::new(uart),
        }
    }
}

impl<U> Deref for UartContext<U>
where
    U: HalUart,
{
    type Target = U;
    fn deref(&self) -> &Self::Target {
        unsafe { &**self.uart.get() }
    }
}

impl<U> Clone for UartContext<U>
where
    U: HalUart,
{
    fn clone(&self) -> Self {
        Self {
            uart: unsafe { UnsafeCell::new(*self.uart.get()) },
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

        peripheral.bind(&irq.clone());
        irq.bind(&peripheral.clone());
        peripheral
    }
}

pub struct UartPeripheral<U>
where
    U: HalUart + 'static,
{
    ctx: UartContext<U>,
    irq: Option<Address<UartInterrupt<U>>>,
    tx_state: TxState,
    rx_state: RxState,

    tx_done: Option<&'static Signal<Result<(), Error>>>,
    rx_done: Option<&'static Signal<Result<usize, Error>>>,
}

impl<U> UartPeripheral<U>
where
    U: HalUart,
{
    pub fn new(ctx: UartContext<U>) -> Self {
        Self {
            tx_done: None,
            rx_done: None,
            ctx,
            irq: None,
            tx_state: TxState::Ready,
            rx_state: RxState::Ready,
        }
    }

    pub fn read<'a>(&'a mut self, rx_buffer: &mut [u8]) -> RxFuture<'a, U> {
        match self.rx_state {
            RxState::Ready => {
                log::trace!("NO RX in progress");
                match self.ctx.read_start(rx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting RX");
                        self.rx_state = RxState::InProgress;
                        RxFuture::new(self, None)
                    }
                    Err(e) => RxFuture::new(self, Some(e)),
                }
            }
            _ => RxFuture::new(self, Some(Error::RxInProgress)),
        }
    }

    pub fn write<'a>(&'a mut self, tx_buffer: &[u8]) -> TxFuture<'a, U> {
        match self.tx_state {
            TxState::Ready => {
                log::trace!("NO TX in progress");
                match self.ctx.write_start(tx_buffer) {
                    Ok(_) => {
                        log::trace!("Starting TX");
                        self.tx_state = TxState::InProgress;
                        TxFuture::new(self, None)
                    }
                    Err(e) => TxFuture::new(self, Some(e)),
                }
            }
            _ => TxFuture::new(self, Some(Error::TxInProgress)),
        }
    }

    fn poll_tx(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        log::trace!("POLL TX");
        if let TxState::InProgress = self.tx_state {
            let tx_done = self.tx_done.unwrap();
            if let Poll::Ready(result) = tx_done.poll_wait(cx) {
                self.tx_state = TxState::Ready;
                log::trace!("Marking TX complete");
                return Poll::Ready(result);
            }
        }

        return Poll::Pending;
    }

    fn poll_rx(&mut self, cx: &mut Context<'_>) -> Poll<Result<usize, Error>> {
        log::trace!("POLL RX");
        if let RxState::InProgress = self.rx_state {
            let rx_done = self.rx_done.unwrap();
            if let Poll::Ready(result) = rx_done.poll_wait(cx) {
                log::trace!("Marking RX complete");
                self.rx_state = RxState::Ready;
                return Poll::Ready(result);
            }
        }

        return Poll::Pending;
    }
}

pub struct UartInterrupt<U>
where
    U: HalUart,
{
    ctx: UartContext<U>,
    tx_done: Signal<Result<(), Error>>,
    rx_done: Signal<Result<usize, Error>>,
}

impl<U> UartInterrupt<U>
where
    U: HalUart,
{
    pub fn new(ctx: UartContext<U>) -> Self {
        Self {
            ctx,
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
        let (tx_done, rx_done) = self.ctx.process_interrupts();
        log::trace!(
            "[UART ISR] TX WAKER: {}. RX WAKER: {}. TX DONE: {}. RX DONE: {}",
            self.tx_done.signaled(),
            self.rx_done.signaled(),
            tx_done,
            rx_done,
        );

        if tx_done {
            self.tx_done.signal(self.ctx.write_finish());
        }

        if rx_done {
            self.rx_done.signal(self.ctx.read_finish());
        }
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

impl<U> Bind<UartInterrupt<U>> for Mutex<UartPeripheral<U>>
where
    U: HalUart,
{
    fn on_bind(&'static mut self, address: Address<UartInterrupt<U>>) {
        self.val.as_mut().unwrap().irq.replace(address);
    }
}

impl<U> Actor for UartPeripheral<U> where U: HalUart {}

pub struct TxFuture<'a, U>
where
    U: HalUart + 'static,
{
    uart: &'a mut UartPeripheral<U>,
    error: Option<Error>,
}

impl<'a, U> TxFuture<'a, U>
where
    U: HalUart,
{
    fn new(uart: &'a mut UartPeripheral<U>, error: Option<Error>) -> Self {
        Self { uart, error }
    }
}

impl<'a, U> Future for TxFuture<'a, U>
where
    U: HalUart,
{
    type Output = Result<(), Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { uart, error } = &mut *self;

        if let Some(e) = &error {
            return Poll::Ready(Err(e.clone()));
        }

        uart.poll_tx(cx)
    }
}

pub struct SetRxWaker(pub Waker);

pub struct RxFuture<'a, U>
where
    U: HalUart + 'static,
{
    uart: &'a mut UartPeripheral<U>,
    error: Option<Error>,
}

impl<'a, U> RxFuture<'a, U>
where
    U: HalUart,
{
    fn new(uart: &'a mut UartPeripheral<U>, error: Option<Error>) -> Self {
        Self { uart, error }
    }
}

impl<'a, U> Future for RxFuture<'a, U>
where
    U: HalUart,
{
    type Output = Result<usize, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { uart, error } = &mut *self;

        if let Some(e) = &error {
            return Poll::Ready(Err(e.clone()));
        }

        uart.poll_rx(cx)
    }
}
