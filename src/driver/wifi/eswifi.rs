use crate::domain::time::duration::Milliseconds;
use crate::driver::spi::SpiController;
use crate::api::arbitrator::BusArbitrator;
use crate::api::delayer::Delayer;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::api::spi::{ChipSelect, SpiBus, SpiError};
use crate::prelude::*;
use core::borrow::BorrowMut;
use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::{consts::*, String};

pub struct Shared {
    ready: AtomicBool,
    ready_waker: RefCell<Option<Waker>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            ready_waker: RefCell::new(None),
        }
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn signal_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
        if let Some(waker) = self.ready_waker.borrow_mut().take() {
            waker.wake()
        }
    }

    fn set_waker(&self, waker: Waker) {
        self.ready_waker.borrow_mut().replace(waker);
    }

    fn clear_waker(&self) {
        self.ready_waker.borrow_mut().take();
    }
}

pub struct EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
{
    shared: Shared,
    controller: ActorContext<EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>>,
    ready: InterruptContext<EsWifiReady<READY>>,
}

impl<SPI, T, CS, READY, RESET, WAKEUP> EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
{
    pub fn new<READY_IRQ: Nr>(
        cs: CS,
        ready: READY,
        ready_irq: READY_IRQ,
        reset: RESET,
        wakeup: WAKEUP,
    ) -> Self {
        Self {
            shared: Shared::new(),
            controller: ActorContext::new(EsWifiController::new(cs, reset, wakeup))
                .with_name("es-wifi"),
            ready: InterruptContext::new(EsWifiReady::new(ready), ready_irq)
                .with_name("es-wifi-irq"),
        }
    }
}

impl<SPI, T, CS, READY, RESET, WAKEUP> Package for EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    READY: InputPin + ExtiPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    type Primary = EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>;
    type Configuration = (Address<BusArbitrator<SPI>>, Address<T>);

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let ready_addr = self.ready.mount(&self.shared, supervisor);
        let controller_addr = self
            .controller
            .mount((config.0, config.1, ready_addr), supervisor);
        controller_addr
    }
}

pub struct EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    ready: READY,
    shared: Option<&'static Shared>,
}

impl<READY> EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    fn new(ready: READY) -> Self {
        Self {
            ready,
            shared: None,
        }
    }
}

impl<READY> Actor for EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    type Configuration = &'static Shared;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config);
    }
}

impl<READY> Interrupt for EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    fn on_interrupt(&mut self) {
        if self.ready.is_high().unwrap_or(false) {
            self.shared.unwrap().signal_ready(true);
        } else {
            self.shared.unwrap().signal_ready(false);
        }
        self.ready.clear_interrupt_pending_bit();
    }
}

struct AwaitReady;
struct QueryReady;

impl<READY> RequestHandler<QueryReady> for EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    type Response = bool;

    fn on_request(self, _message: QueryReady) -> Response<Self, Self::Response> {
        let ready = self.shared.unwrap().is_ready();
        Response::immediate(self, ready)
    }
}

impl<READY> RequestHandler<AwaitReady> for EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    type Response = ();

    fn on_request(mut self, message: AwaitReady) -> Response<Self, Self::Response> {
        if self.ready.is_high().unwrap_or(false) {
            self.shared.unwrap().borrow_mut().signal_ready(true);
            Response::immediate(self, ())
        } else {
            let future = AwaitReadyFuture::new(self.shared.unwrap());
            Response::immediate_future(self, future)
        }
    }
}

struct AwaitReadyFuture {
    waiting: bool,
    shared: &'static Shared,
}

impl AwaitReadyFuture {
    fn new(shared: &'static Shared) -> Self {
        Self {
            waiting: false,
            shared,
        }
    }
}

impl Future for AwaitReadyFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.shared.is_ready() {
            self.shared.clear_waker();
            Poll::Ready(())
        } else {
            if !self.waiting {
                self.shared.set_waker(cx.waker().clone());
                self.waiting = true;
            }
            Poll::Pending
        }
    }
}

enum State {
    Uninitialized,
    Ready,
}

pub struct EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    spi: Option<Address<BusArbitrator<SPI>>>,
    delayer: Option<Address<T>>,
    ready: Option<Address<EsWifiReady<READY>>>,
    cs: ChipSelect<CS, T>,
    reset: RESET,
    wakeup: WAKEUP,
    state: State,
}

impl<SPI, T, CS, READY, RESET, WAKEUP> EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    pub fn new(cs: CS, reset: RESET, wakeup: WAKEUP) -> Self {
        Self {
            spi: None,
            delayer: None,
            ready: None,
            cs: ChipSelect::new(cs, Milliseconds(5u32)),
            reset,
            wakeup,
            state: State::Uninitialized,
        }
    }

    async fn wakeup(&mut self) {
        self.wakeup.set_low();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
        self.wakeup.set_high();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
    }

    async fn reset(&mut self) {
        self.reset.set_low();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
        self.reset.set_high();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
    }
}

macro_rules! command {
    ($size:tt, $($arg:tt)*) => ({
        //let mut c = String::new();
        //c
        let mut c = String::<$size>::new();
        write!(c, $($arg)*);
        c.push_str("\r");
        c
    })
}

const NAK: u8 = 0x15;

impl<SPI, T, CS, READY, RESET, WAKEUP> Actor for EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    READY: InputPin + ExtiPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    type Configuration = (
        Address<BusArbitrator<SPI>>,
        Address<T>,
        Address<EsWifiReady<READY>>,
    );

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.spi.replace(config.0);
        self.delayer.replace(config.1);
        self.ready.replace(config.2);
        self.cs.set_delayer(config.1);
    }

    fn on_start(mut self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(async move {
            log::info!("[{}] start", ActorInfo::name());
            self.reset().await;
            self.wakeup().await;
            let mut spi = self.spi.unwrap().begin_transaction().await;

            let mut response = [0 as u8; 16];
            let mut pos = 0;

            self.ready.unwrap().request(AwaitReady {}).await;
            {
                let cs = self.cs.select().await;

                loop {
                    if !self.ready.unwrap().request(QueryReady {}).await {
                        break;
                    }
                    if pos >= response.len() {
                        break;
                    }
                    let mut chunk = [0x0A, 0x0A];
                    spi.spi_transfer(&mut chunk).await;
                    // reverse order going from 16 -> 2*8 bits
                    if chunk[1] != NAK {
                        response[pos] = chunk[1];
                        pos += 1;
                    }
                    if chunk[0] != NAK {
                        response[pos] = chunk[0];
                        pos += 1;
                    }
                }
            }

            let needle = &[b'\r', b'\n', b'>', b' '];

            if !response[0..pos].starts_with(needle) {
                log::info!(
                    "[{}] failed to initialize {:?}",
                    ActorInfo::name(),
                    &response[0..pos]
                );
            } else {
                // disable verbosity
                //self.send_string(&command!(U8, "MT=1"), &mut response);
                self.state = State::Ready;
                log::info!("[{}] eS-WiFi adapter is ready", ActorInfo::name());
            }
            (self)
        })
    }
}
