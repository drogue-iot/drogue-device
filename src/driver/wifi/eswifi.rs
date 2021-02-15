use crate::domain::time::duration::Milliseconds;
use crate::driver::delayer::Delayer;
use crate::driver::spi::SpiController;
use crate::hal::arbitrator::BusArbitrator;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::hal::spi::{SpiBus, SpiError};
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
            controller: ActorContext::new(EsWifiController::new(cs, reset, wakeup)),
            ready: InterruptContext::new(EsWifiReady::new(ready), ready_irq),
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
            log::info!("[eswifi-ready] is ready IS HIGH");
            self.shared.unwrap().signal_ready(true);
        } else {
            log::info!("[eswifi-ready] is not ready IS LOW");
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

pub struct EsWifiController<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    spi: Option<Address<BusArbitrator<SPI>>>,
    delayer: Option<Address<T>>,
    ready: Option<Address<EsWifiReady<READY>>>,
    cs: CS,
    reset: RESET,
    wakeup: WAKEUP,
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
            cs,
            reset,
            wakeup,
        }
    }

    async fn wakeup(&mut self) {
        log::info!("wake-up set low");
        self.wakeup.set_low();
        self.delayer.unwrap().delay(Milliseconds(500u32)).await;
        log::info!("wake-up set high");
        self.wakeup.set_high();
        self.delayer.unwrap().delay(Milliseconds(500u32)).await;
    }

    async fn reset(&mut self) {
        log::info!("reset set low");
        self.reset.set_low();
        self.delayer.unwrap().delay(Milliseconds(500u32)).await;
        log::info!("reset set high");
        self.reset.set_high();
        self.delayer.unwrap().delay(Milliseconds(500u32)).await;
    }
}

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
    }

    fn on_start(mut self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(async move {
            log::info!("[es-wifi] start");
            //self.cs.set_high();
            //self.delayer.unwrap().delay(Milliseconds(1000u32)).await;
            self.reset().await;
            self.wakeup().await;
            log::info!("starting spi transaction");
            let mut spi = self.spi.unwrap().begin_transaction().await;
            log::info!("[es-wifi] began SPI");
            self.ready.unwrap().request(AwaitReady {}).await;
            log::info!("[es-wifi] ready to go");
            self.cs.set_low();
            self.delayer.unwrap().delay(Milliseconds(500u32)).await;
            log::info!("[es-wifi] CS set low");
            /*
            loop {
                if !self.ready.unwrap().request(QueryReady {}).await {
                    log::info!("no longer ready");
                    break;
                }
                let mut chunk = [0x0A, 0x0A];
                spi.transfer(&mut chunk).await;
                log::info!("chunk {:?}", chunk);
            }
             */
            let mut response = [0 as u8; 16];
            let mut pos = 0;

            loop {
                //log::info!("loop {}", pos);
                if !self.ready.unwrap().request(QueryReady {}).await {
                    break;
                }
                if pos >= response.len() {
                    log::info!("***************** overrun");
                    //return Err(());
                    break;
                }
                let mut chunk = [0x0A, 0x0A];
                spi.spi_transfer(&mut chunk).await;
                log::info!("transfer {:?}", chunk);
                // reverse order going from 16 -> 2*8 bits
                if chunk[1] != 0x15 {
                    response[pos] = chunk[1];
                    pos += 1;
                }
                if chunk[0] != 0x15 {
                    response[pos] = chunk[0];
                    pos += 1;
                }
            }
            log::info!("[es-wifi] end transfer");
            (self)
        })
    }
}
