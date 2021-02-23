use crate::api::scheduler::*;
use crate::api::uart::*;
use crate::domain::time::duration::{Duration, Milliseconds};
use crate::hal::gpio::InterruptPin;
use crate::prelude::*;
use crate::util::dma::async_bbqueue::{consts, AsyncBBBuffer, AsyncBBConsumer, AsyncBBProducer};

use super::common::*;
use core::cell::UnsafeCell;
use cortex_m::interrupt::Nr;
use embedded_hal::serial::{Read, Write};
use nb;

pub struct Serial<TX, RX, RxPin, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
    S: Scheduler + 'static,
{
    actor: ActorContext<SerialActor<TX, S>>,
    interrupt: InterruptContext<SerialInterrupt<RX, RxPin>>,
    state: ActorState,
    rx_buffer: UnsafeCell<AsyncBBBuffer<'static, consts::U16>>,
}

pub struct SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Scheduler + 'static,
{
    me: Option<Address<Self>>,
    tx: TX,
    rx_consumer: Option<AsyncBBConsumer<consts::U16>>,
    state: Option<&'static ActorState>,
    scheduler: Option<Address<S>>,
}

pub struct SerialInterrupt<RX, RxPin>
where
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
{
    rx: RX,
    rx_pin: RxPin,
    rx_producer: Option<AsyncBBProducer<consts::U16>>,
    state: Option<&'static ActorState>,
}

impl<TX, RX, RxPin, S> Serial<TX, RX, RxPin, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
    S: Scheduler + 'static,
{
    pub fn new<IRQ>(tx: TX, rx: RX, rx_pin: RxPin, irq: IRQ) -> Self
    where
        IRQ: Nr,
    {
        Self {
            state: ActorState::new(),
            rx_buffer: UnsafeCell::new(AsyncBBBuffer::new()),
            actor: ActorContext::new(SerialActor {
                me: None,
                tx,
                rx_consumer: None,
                state: None,
                scheduler: None,
            }),
            interrupt: InterruptContext::new(
                SerialInterrupt {
                    rx,
                    rx_pin,
                    rx_producer: None,
                    state: None,
                },
                irq,
            ),
        }
    }
}

impl<TX, RX, RxPin, S> Package for Serial<TX, RX, RxPin, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
    S: Scheduler + 'static,
{
    type Primary = SerialActor<TX, S>;
    type Configuration = Address<S>;
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let (rx_prod, rx_cons) = unsafe { (&mut *self.rx_buffer.get()).split() };

        let addr = self.actor.mount((&self.state, config, rx_cons), supervisor);
        self.interrupt.mount((&self.state, rx_prod), supervisor);

        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

impl<TX, S> Actor for SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Scheduler + 'static,
{
    type Configuration = (
        &'static ActorState,
        Address<S>,
        AsyncBBConsumer<consts::U16>,
    );
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.me.replace(me);
        self.state.replace(config.0);
        self.scheduler.replace(config.1);
        self.rx_consumer.replace(config.2);
    }
}

impl<RX, RxPin> Actor for SerialInterrupt<RX, RxPin>
where
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
{
    type Configuration = (&'static ActorState, AsyncBBProducer<consts::U16>);
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.state.replace(config.0);
        self.rx_producer.replace(config.1);
    }

    fn on_start(mut self) -> Completion<Self> {
        self.rx_pin.enable_interrupt();
        Completion::immediate(self)
    }
}

impl<RX, RxPin> Interrupt for SerialInterrupt<RX, RxPin>
where
    RX: Read<u8> + 'static,
    RxPin: InterruptPin + 'static,
{
    fn on_interrupt(&mut self) {
        if self.rx_pin.check_interrupt() {
            match self.rx.read() {
                Ok(b) => {
                    if let Ok(mut grant) = self.rx_producer.as_ref().unwrap().prepare_write(1) {
                        grant.buf()[0] = b;
                    } else {
                        log::warn!("Dropping data!");
                    }
                }
                Err(e) => {}
            }
        }
        self.rx_pin.clear_interrupt();
    }
}

impl<TX, S> UartWriter for SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Scheduler + 'static,
{
    fn write<'a>(mut self, message: UartWrite<'a>) -> Response<Self, Result<(), Error>> {
        unsafe {
            Response::defer_unchecked(async move {
                for b in message.0 {
                    loop {
                        match self.tx.write(*b) {
                            Err(nb::Error::WouldBlock) => continue,
                            Err(nb::Error::Other(e)) => return (self, Err(Error::Transmit)),
                            _ => {}
                        }
                    }
                }
                (self, Ok(()))
            })
        }
    }
}

impl<TX, S> UartReader for SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Scheduler + 'static,
{
    fn read<'a>(self, message: UartRead<'a>) -> Response<Self, Result<usize, Error>> {
        let state = self.state.as_ref().unwrap();
        if state.try_rx_busy() {
            let rx_consumer = self.rx_consumer.as_ref().unwrap();
            let future = unsafe { rx_consumer.read(message.0) };
            let future = RxFuture::new(future, state);
            Response::immediate_future(self, future)
        } else {
            Response::immediate(self, Err(Error::RxInProgress))
        }
    }

    fn read_with_timeout<'a, DUR>(
        self,
        message: UartReadWithTimeout<'a, DUR>,
    ) -> Response<Self, Result<usize, Error>>
    where
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        let state = self.state.as_ref().unwrap();
        if state.try_rx_busy() {
            let rx_consumer = self.rx_consumer.as_ref().unwrap();
            let future = unsafe { rx_consumer.read(message.0) };
            let future = RxFuture::new(future, state);
            state.reset_rx_timeout();
            self.scheduler.as_ref().unwrap().schedule(
                message.1,
                ReadTimeout,
                *self.me.as_ref().unwrap(),
            );
            Response::immediate_future(self, future)
        } else {
            Response::immediate(self, Err(Error::RxInProgress))
        }
    }
}

impl<TX, S> NotifyHandler<ReadTimeout> for SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Scheduler + 'static,
{
    fn on_notify(self, message: ReadTimeout) -> Completion<Self> {
        let state = self.state.as_ref().unwrap();
        state.signal_rx_timeout();
        Completion::immediate(self)
    }
}

#[derive(Clone)]
struct ReadTimeout;
