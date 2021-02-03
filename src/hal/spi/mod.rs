use crate::address::Address;
use crate::bus::EventBus;
use crate::prelude::*;
use crate::synchronization::Mutex;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;
use embedded_hal::spi::FullDuplex;
use nb::Error;

pub struct Spi<SPI: FullDuplex<u8> + 'static> {
    mutex: ActorContext<Mutex<SpiPeripheral<SPI>>>,
    irq: InterruptContext<SpiInterrupt>,
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

impl<SPI> Bind<SpiInterrupt> for Mutex<SpiPeripheral<SPI>>
where
    SPI: FullDuplex<u8>,
{
    fn on_bind(&mut self, address: Address<SpiInterrupt>) {
        self.val.as_mut().unwrap().irq.replace(address);
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
        periph_addr.bind(&irq_addr);
        periph_addr
    }
}

pub struct SpiInterrupt {
    waker: Option<Waker>,
}

impl SpiInterrupt {
    fn new() -> Self {
        Self { waker: None }
    }

    fn signal(&mut self) {
        if let Some(ref waker) = self.waker {
            waker.wake_by_ref()
        }
    }
}

struct SetWaker(Waker);

impl NotifyHandler<SetWaker> for SpiInterrupt {
    fn on_notify(&'static mut self, message: SetWaker) -> Completion<Self> {
        self.waker.replace(message.0);
        Completion::immediate(self)
    }
}

impl Actor for SpiInterrupt {}

impl Interrupt for SpiInterrupt {
    fn on_interrupt(&mut self) {
        self.signal();
    }
}

pub struct SpiPeripheral<SPI>
where
    SPI: FullDuplex<u8> + 'static,
{
    spi: SPI,
    irq: Option<Address<SpiInterrupt>>,
}

impl<SPI: FullDuplex<u8>> SpiPeripheral<SPI> {
    pub fn new(spi: SPI) -> Self {
        Self { spi, irq: None }
    }

    pub fn transfer<'w>(&'w mut self, buf: &'w mut [u8]) -> TransferFuture<'w, SPI> {
        TransferFuture::new(self, buf)
    }

    fn poll_transfer(
        &mut self,
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
                            *state = State::Read(*index);
                        }
                        Err(e) => {
                            match e {
                                Error::Other(e) => {
                                    // failed.
                                    return Poll::Ready(Err(e));
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
                            *state = State::Write(*index + 1);
                        }
                        Err(e) => {
                            match e {
                                Error::Other(e) => {
                                    // failed.
                                    return Poll::Ready(Err(e));
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

pub struct TransferFuture<'w, SPI: FullDuplex<u8> + 'static> {
    spi: &'w mut SpiPeripheral<SPI>,
    buf: &'w mut [u8],
    state: State,
}

impl<'w, SPI: FullDuplex<u8>> TransferFuture<'w, SPI> {
    pub fn new(spi: &'w mut SpiPeripheral<SPI>, buf: &'w mut [u8]) -> Self {
        Self {
            spi,
            buf,
            state: State::Write(0),
        }
    }
}
impl<'w, SPI: FullDuplex<u8>> Future for TransferFuture<'w, SPI> {
    type Output = Result<(), SPI::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            spi: transfer,
            buf,
            state,
        } = &mut *self;
        transfer.poll_transfer(cx, buf, state)
    }
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
    fn start(&'static mut self) -> Completion<Self> {
        Completion::defer(async move {
            let mut periph = self.spi.lock().await;
            let mut buf = [0; 16];
            let result = periph.transfer(&mut buf).await;
            // prove we can borrow immutable afterwards.
            use_it(&buf);

            // prove we can borrow mutable afterwards.
            use_it_mut(&mut buf);
            self
        })
    }
}

pub fn use_it(buf: &[u8]) {}
pub fn use_it_mut(buf: &mut [u8]) {}
