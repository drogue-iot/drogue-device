use crate::actor::{Actor, ActorContext};
use crate::address::Address;
use crate::bind::Bind;
use crate::bus::EventBus;
use crate::device::Device;
use crate::prelude::*;

pub use crate::hal::uart::Error;
use crate::hal::uart::Uart as HalUart;
use crate::handler::{Completion, NotifyHandler, RequestHandler, Response};
use crate::interrupt::{Interrupt, InterruptContext};
use crate::package::Package;
use crate::synchronization::Mutex;

use core::cell::UnsafeCell;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;

pub struct Uart<U>
where
    U: HalUart + 'static,
{
    peripheral: ActorContext<Mutex<UartPeripheral<U>>>,
    irq: InterruptContext<UartInterrupt>,
}
/*
}*/

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
        Self {
            peripheral: ActorContext::new(Mutex::new(UartPeripheral::new(uart))),
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

        peripheral.bind(&irq.clone());
        //irq.bind(&peripheral.clone());
        peripheral
    }
}

pub struct UartPeripheral<U>
where
    U: HalUart + 'static,
{
    uart: U,
    irq: Option<Address<UartInterrupt>>,
    tx_state: TxState,
    rx_state: RxState,
}

impl<U> UartPeripheral<U>
where
    U: HalUart,
{
    pub fn new(uart: U) -> Self {
        Self {
            uart,
            irq: None,
            tx_state: TxState::Ready,
            rx_state: RxState::Ready,
        }
    }

    pub fn read<'a>(&'a mut self, rx_buffer: &mut [u8]) -> RxFuture<'a, U> {
        log::info!("READ!");
        match self.rx_state {
            RxState::Ready => {
                log::trace!("NO RX in progress");
                match self.uart.read_start(rx_buffer) {
                    Ok(_) => {
                        log::info!("Starting RX");
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
        log::info!("WRITE!");
        match self.tx_state {
            TxState::Ready => {
                log::trace!("NO TX in progress");
                match self.uart.write_start(tx_buffer) {
                    Ok(_) => {
                        log::info!("Starting TX");
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
        log::info!("POLL TX");
        if let TxState::InProgress = self.tx_state {
            if self.uart.write_done() {
                log::info!("Marking TX complete");
                self.tx_state = TxState::Ready;
                return Poll::Ready(self.uart.write_finish());
            } else {
                log::info!("Setting TX WAKER");
                self.irq
                    .as_ref()
                    .unwrap()
                    .notify(SetTxWaker(cx.waker().clone()));
            }
        }

        return Poll::Pending;
    }

    fn poll_rx(&mut self, cx: &mut Context<'_>) -> Poll<Result<usize, Error>> {
        log::info!("POLL RX");
        if let RxState::InProgress = self.rx_state {
            if self.uart.read_done() {
                log::info!("Marking RX complete");
                self.rx_state = RxState::Ready;
                return Poll::Ready(self.uart.read_finish());
            } else {
                self.irq
                    .as_ref()
                    .unwrap()
                    .notify(SetRxWaker(cx.waker().clone()));
            }
        }

        return Poll::Pending;
    }
}

pub struct UartInterrupt {
    tx_waker: Option<Waker>,
    rx_waker: Option<Waker>,
}

impl UartInterrupt {
    pub fn new() -> Self {
        Self {
            tx_waker: None,
            rx_waker: None,
        }
    }
}

impl Actor for UartInterrupt {}

impl Interrupt for UartInterrupt {
    fn on_interrupt(&mut self) {
        log::info!("UART INTERRUPT");
        if let Some(ref waker) = self.tx_waker {
            log::info!("WAKEUP TX");
            waker.wake_by_ref();
        }

        if let Some(ref waker) = self.rx_waker {
            log::info!("WAKEUP RX");
            waker.wake_by_ref();
        }
    }
}

impl NotifyHandler<SetRxWaker> for UartInterrupt {
    fn on_notify(&'static mut self, waker: SetRxWaker) -> Completion {
        log::info!("SET RX WAKER");
        self.rx_waker.replace(waker.0);
        Completion::immediate()
    }
}

impl NotifyHandler<SetTxWaker> for UartInterrupt {
    fn on_notify(&'static mut self, waker: SetTxWaker) -> Completion {
        log::info!("SET TX WAKER");
        self.tx_waker.replace(waker.0);
        Completion::immediate()
    }
}

/*
impl<U> Bind<Mutex<UartPeripheral<U>>> for UartInterrupt<U>
where
    U: HalUart,
{
    fn on_bind(&'static mut self, address: Address<Mutex<UartPeripheral<U>>>) {
        self.peripheral.replace(address);
    }
}*/

impl<U> Bind<UartInterrupt> for Mutex<UartPeripheral<U>>
where
    U: HalUart,
{
    fn on_bind(&'static mut self, address: Address<UartInterrupt>) {
        self.val.as_mut().unwrap().irq.replace(address);
    }
}

impl<U> Actor for UartPeripheral<U> where U: HalUart {}

pub struct SetTxWaker(pub Waker);

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
        log::info!("TX: SOMEONE WANTED TO POLL?");

        if let Some(e) = &error {
            return Poll::Ready(Err(e.clone()));
        }

        let r = uart.poll_tx(cx);
        log::info!("TX POLL: {:?}", r);
        r
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
        log::info!("SOMEONE WANTED TO POLL?");

        if let Some(e) = &error {
            return Poll::Ready(Err(e.clone()));
        }

        uart.poll_rx(cx)
    }
}
