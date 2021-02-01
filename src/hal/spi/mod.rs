use crate::address::Address;
use crate::bus::EventBus;
use crate::prelude::*;
use crate::synchronization::{Exclusive, Lock, Mutex, Unlock};

use crate::handler::{RequestHandler, Response};
use core::cell::UnsafeCell;
use core::future::Future;
use core::marker::PhantomData;
use core::mem::replace;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;
use embedded_hal::spi::FullDuplex;
use nb::Error;

pub struct Spi<SPI: FullDuplex<u8> + 'static> {
    mutex: ActorContext<Mutex<SpiPeripheral<SPI>>>,
    irq: InterruptContext<SpiInterrupt<SPI>>,
}

impl<SPI> Spi<SPI>
where
    SPI: FullDuplex<u8>,
{
    pub fn new<IRQ: Nr>(spi: SPI, irq: IRQ) -> Self {
        Self {
            mutex: ActorContext::new(Mutex::new(SpiPeripheral::new(spi))),
            irq: InterruptContext::new(SpiInterrupt::new(), irq),
        }
    }
}

impl<SPI> Bind<SpiInterrupt<SPI>> for Mutex<SpiPeripheral<SPI>>
where
    SPI: FullDuplex<u8>,
{
    fn on_bind(&'static mut self, address: Address<SpiInterrupt<SPI>>) {
        self.val.as_mut().unwrap().irq.replace(address);
    }
}

impl<SPI> Bind<Mutex<SpiPeripheral<SPI>>> for SpiInterrupt<SPI>
where
    SPI: FullDuplex<u8>,
{
    fn on_bind(&'static mut self, address: Address<Mutex<SpiPeripheral<SPI>>>) {
        address.notify(SetFlags {
            tx_ready: &self.tx_ready,
            rx_ready: &self.rx_ready,
        });
    }
}

impl<SPI> NotifyHandler<SetFlags> for Mutex<SpiPeripheral<SPI>>
where
    SPI: FullDuplex<u8>,
{
    fn on_notify(&'static mut self, message: SetFlags) -> Completion {
        self.val
            .as_mut()
            .unwrap()
            .tx_ready
            .replace(message.tx_ready);
        self.val
            .as_mut()
            .unwrap()
            .rx_ready
            .replace(message.rx_ready);
        Completion::immediate()
    }
}

impl<D, SPI> Package<D, Mutex<SpiPeripheral<SPI>>> for Spi<SPI>
where
    D: Device,
    SPI: FullDuplex<u8>,
{
    fn mount(
        &'static self,
        bus_address: &Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<Mutex<SpiPeripheral<SPI>>> {
        let periph_addr = self.mutex.mount(supervisor);
        let irq_addr = self.irq.mount(supervisor);
        periph_addr.bind(&irq_addr.clone());
        irq_addr.bind(&periph_addr.clone());
        periph_addr
    }
}

pub struct SpiInterrupt<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    tx_ready: AtomicBool,
    rx_ready: AtomicBool,
    waker: Option<Waker>,
    _marker: PhantomData<SPI>,
}

impl<SPI> SpiInterrupt<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    fn new() -> Self {
        Self {
            tx_ready: AtomicBool::new(true),
            rx_ready: AtomicBool::new(false),
            waker: None,
            _marker: PhantomData,
        }
    }

    fn signal_rxne(&mut self) {
        self.rx_ready.store(true, Ordering::Release);
        if let Some(ref waker) = self.waker {
            waker.wake_by_ref()
        }
    }

    fn signal_txe(&mut self) {
        self.tx_ready.store(true, Ordering::Release);
        if let Some(ref waker) = self.waker {
            waker.wake_by_ref()
        }
    }
}

struct SetWaker(Waker);

impl<SPI> NotifyHandler<SetWaker> for SpiInterrupt<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    fn on_notify(&'static mut self, message: SetWaker) -> Completion {
        self.waker.replace(message.0.clone());
        Completion::immediate()
    }
}

impl<SPI> Actor for SpiInterrupt<SPI> where SPI: FullDuplex<u8> {}

impl<SPI> Interrupt for SpiInterrupt<SPI>
where
    SPI: FullDuplex<u8>,
{
    fn on_interrupt(&mut self) {
        // if rxne -> signal rx_ready
        self.signal_rxne();
        // if txe -> signal tx_ready
        self.signal_txe();
    }
}

struct SetFlags {
    tx_ready: &'static AtomicBool,
    rx_ready: &'static AtomicBool,
}

pub struct SpiPeripheral<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    spi: SPI,
    irq: Option<Address<SpiInterrupt<SPI>>>,
    tx_ready: Option<&'static AtomicBool>,
    rx_ready: Option<&'static AtomicBool>,
}

impl<SPI: FullDuplex<u8>> SpiPeripheral<SPI> {
    pub fn new(spi: SPI) -> Self {
        Self {
            spi,
            irq: None,
            tx_ready: None,
            rx_ready: None,
        }
    }

    pub fn transfer<'w>(&'w mut self, buf: &'w mut [u8]) -> TransferFuture<'w, Self> {
        TransferFuture::new(self, buf)
    }
}

impl<SPI: FullDuplex<u8>> Transfer for SpiPeripheral<SPI> {
    type Error = SPI::Error;

    fn poll_transfer(
        self: &mut Self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        state: &mut State,
    ) -> Poll<Result<(), SPI::Error>> {
        loop {
            match state {
                State::Write(ref index) => {
                    if index + 1 > buf.len() {
                        return Poll::Ready(Ok(()));
                    }
                    let result = self.spi.send(buf[*index]);
                    match result {
                        Ok(_) => {
                            // sent, next is read, keep going!
                            replace(state, State::Read(*index));
                        }
                        Err(e) => {
                            match e {
                                Error::Other(e) => {
                                    // failed.
                                    return Poll::Ready( Err(e) );
                                }
                                Error::WouldBlock => {
                                    // we made no progress,
                                    self.irq
                                        .as_ref()
                                        .unwrap()
                                        .notify(SetWaker(cx.waker().clone()));
                                    return Poll::Pending;
                                }
                            }
                        }
                    }
                }
                State::Read(ref index) => {
                    let result = self.spi.read();
                    match result {
                        Ok(word) => {
                            buf[*index] = word;
                            replace(state, State::Write(*index + 1));
                        }
                        Err(e) => {
                            match e {
                                Error::Other(e) => {
                                    // failed.
                                    return Poll::Ready( Err(e) );
                                }
                                Error::WouldBlock => {
                                    self.irq
                                        .as_ref()
                                        .unwrap()
                                        .notify(SetWaker(cx.waker().clone()));
                                    return Poll::Pending;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<SPI: FullDuplex<u8>> Unpin for SpiPeripheral<SPI> {}

pub enum State {
    Write(usize),
    Read(usize),
}

pub struct TransferFuture<'w, T: Transfer + Unpin + ?Sized> {
    transfer: &'w mut T,
    buf: &'w mut [u8],
    state: State,
}

impl<'w, T: Transfer + Unpin + ?Sized> TransferFuture<'w, T> {
    pub fn new(transfer: &'w mut T, buf: &'w mut [u8]) -> Self {
        Self {
            transfer,
            buf,
            state: State::Write(0),
        }
    }
}

impl<T: Transfer + Unpin + ?Sized> Future for TransferFuture<'_, T> {
    type Output = Result<(), T::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            transfer,
            buf,
            state,
        } = &mut *self;
        transfer.poll_transfer(cx, buf, state)
    }
}

pub trait Transfer {
    type Error;
    fn poll_transfer(
        self: &mut Self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        state: &mut State,
    ) -> Poll<Result<(), Self::Error>>;
}

// ------------------------------------------------------------------------
// ------------------------------------------------------------------------

struct TestActor<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    spi: Address<Mutex<SpiPeripheral<SPI>>>,
}

impl<SPI> Actor for TestActor<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    fn start(&'static mut self) -> Completion {
        Completion::defer(async move {
            let mut periph = self.spi.lock().await;
            let mut buf = [0; 16];
            let result = periph.transfer(&mut buf).await;
            // prove we can borrow immutable afterwards.
            use_it(&buf);

            // prove we can borrow mutable afterwards.
            use_it_mut(&mut buf);
        })
    }
}

pub fn use_it(buf: &[u8]) {}
pub fn use_it_mut(buf: &mut [u8]) {}
