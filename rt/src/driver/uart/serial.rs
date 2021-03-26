use crate::api::timer::*;
use crate::api::uart::*;
use crate::domain::time::duration::{Duration, Milliseconds};

use crate::hal::uart::UartRx;
use crate::prelude::*;
use crate::util::dma::async_bbqueue::{consts, AsyncBBBuffer, AsyncBBConsumer, AsyncBBProducer};

use super::common::*;
use core::cell::UnsafeCell;
use cortex_m::interrupt::Nr;
use embedded_hal::serial::{Read, Write};
use nb;

pub struct Serial<TX, RX, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + UartRx + 'static,
    S: Timer + 'static,
{
    actor: ActorContext<SerialActor<TX, S>>,
    interrupt: InterruptContext<SerialInterrupt<RX>>,
    state: ActorState,
    rx_buffer: UnsafeCell<AsyncBBBuffer<'static, consts::U1024>>,
}

pub struct SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Timer + 'static,
{
    me: Option<Address<Self>>,
    tx: TX,
    rx_consumer: Option<AsyncBBConsumer<consts::U1024>>,
    state: Option<&'static ActorState>,
    scheduler: Option<Address<S>>,
}

pub struct SerialInterrupt<RX>
where
    RX: Read<u8> + UartRx + 'static,
{
    rx: RX,
    rx_producer: Option<AsyncBBProducer<consts::U1024>>,
    state: Option<&'static ActorState>,
}

impl<TX, RX, S> Serial<TX, RX, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + UartRx + 'static,
    S: Timer + 'static,
{
    pub fn new<IRQ>(tx: TX, rx: RX, irq: IRQ) -> Self
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
                    rx_producer: None,
                    state: None,
                },
                irq,
            ),
        }
    }
}

impl<TX, RX, S> Package for Serial<TX, RX, S>
where
    TX: Write<u8> + 'static,
    RX: Read<u8> + UartRx + 'static,
    S: Timer + 'static,
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

impl<'a, TX, S> Actor for SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Timer + 'static,
{
    type Configuration = (
        &'static ActorState,
        Address<S>,
        AsyncBBConsumer<consts::U1024>,
    );
    type Request = UartRequest<'a>;
    type Response = UartResponse;

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.me.replace(me);
        self.state.replace(config.0);
        self.scheduler.replace(config.1);
        self.rx_consumer.replace(config.2);
    }

    fn on_request(self, request: Self::Request) -> Response<Self> {
        match request {
            UartRequest::Write(buf) => {
                let result = self.write_str(buf);
                Response::immediate(self, result)
            }
            UartRequest::Read(buf) => {
                let state = self.state.as_ref().unwrap();
                if state.try_rx_busy() {
                    let rx_consumer = self.rx_consumer.as_ref().unwrap();
                    let future = unsafe { rx_consumer.read(buf) };
                    let future = RxFuture::new(future, state);
                    Response::immediate_future(self, future)
                } else {
                    Response::immediate(self, Err(Error::RxInProgress))
                }
            }
        }
    }
}

impl<RX> Actor for SerialInterrupt<RX>
where
    RX: Read<u8> + UartRx + 'static,
{
    type Configuration = (&'static ActorState, AsyncBBProducer<consts::U1024>);
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.state.replace(config.0);
        self.rx_producer.replace(config.1);
    }

    fn on_start(mut self) -> Completion<Self> {
        self.rx.enable_interrupt();
        Completion::immediate(self)
    }
}

impl<RX> Interrupt for SerialInterrupt<RX>
where
    RX: Read<u8> + UartRx + 'static,
{
    fn on_interrupt(&mut self) {
        if self.rx.check_interrupt() {
            if let Ok(mut grant) = self.rx_producer.as_ref().unwrap().prepare_write(1) {
                let buf = grant.buf();
                let mut i = 0;
                while i < buf.len() {
                    match self.rx.read() {
                        Ok(b) => {
                            buf[i] = b;
                            i += 1;
                        }
                        Err(nb::Error::WouldBlock) => {
                            break;
                        }
                        Err(e) => {
                            log::warn!("Error while reading");
                            break;
                        }
                    }
                }
                grant.commit(i);
            }
        }
        self.rx.clear_interrupt();
    }
}

impl<TX, S> SerialActor<TX, S>
where
    TX: Write<u8> + 'static,
    S: Timer + 'static,
{
    fn write_str(&mut self, buf: &[u8]) -> Result<(), Error> {
        for b in buf.iter() {
            loop {
                match self.tx.write(*b) {
                    Err(nb::Error::WouldBlock) => {
                        nb::block!(self.tx.flush()).map_err(|_| Error::Transmit)?;
                    }
                    Err(_) => return Err(Error::Transmit),
                    _ => break,
                }
            }
        }
        nb::block!(self.tx.flush()).map_err(|_| Error::Transmit)?;
        Ok(())
    }
}
