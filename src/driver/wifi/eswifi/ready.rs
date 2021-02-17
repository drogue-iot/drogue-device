use crate::hal::gpio::exti_pin::ExtiPin;
use crate::prelude::*;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::InputPin;

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
        cortex_m::interrupt::free(|cs| {
            self.ready.store(ready, Ordering::Release);
            if let Some(waker) = self.ready_waker.borrow_mut().take() {
                waker.wake()
            }
        })
    }

    fn set_waker(&self, waker: Waker) -> bool {
        cortex_m::interrupt::free(|cs| {
            if self.is_ready() {
                waker.wake();
                true
            } else {
                self.ready_waker.borrow_mut().replace(waker);
                false
            }
        })
    }

    fn clear_waker(&self) {
        cortex_m::interrupt::free(|cs| {
            self.ready_waker.borrow_mut().take();
        })
    }
}

pub struct EsWifiReady<READY>
where
    READY: InputPin + ExtiPin + 'static,
{
    shared: Shared,
    ready: ActorContext<EsWifiReadyPin>,
    irq: InterruptContext<EsWifiReadyInterrupt<READY>>,
}

impl<READY> EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    pub fn new<IRQ: Nr>(ready: READY, irq: IRQ) -> Self {
        Self {
            shared: Shared::new(),
            ready: ActorContext::new(EsWifiReadyPin::new()),
            irq: InterruptContext::new(EsWifiReadyInterrupt::new(ready), irq),
        }
    }
}

impl<READY> Package for EsWifiReady<READY>
where
    READY: InputPin + ExtiPin,
{
    type Primary = EsWifiReadyPin;
    type Configuration = ();

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let addr = self.ready.mount(&self.shared, supervisor);
        self.irq.mount(&self.shared, supervisor);
        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.ready.address()
    }
}

pub struct EsWifiReadyPin {
    shared: Option<&'static Shared>,
}

impl EsWifiReadyPin {
    pub fn new() -> Self {
        Self { shared: None }
    }
}

impl Actor for EsWifiReadyPin {
    type Configuration = &'static Shared;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config);
    }
}

pub struct EsWifiReadyInterrupt<READY>
where
    READY: InputPin + ExtiPin,
{
    ready: READY,
    shared: Option<&'static Shared>,
}

impl<READY> EsWifiReadyInterrupt<READY>
where
    READY: InputPin + ExtiPin,
{
    pub fn new(ready: READY) -> Self {
        Self {
            ready,
            shared: None,
        }
    }
}

impl<READY> Actor for EsWifiReadyInterrupt<READY>
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

impl<READY> Interrupt for EsWifiReadyInterrupt<READY>
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

pub struct AwaitReady;
pub struct QueryReady;

impl RequestHandler<QueryReady> for EsWifiReadyPin {
    type Response = bool;

    fn on_request(mut self, _message: QueryReady) -> Response<Self, Self::Response> {
        let ready = self.shared.unwrap().is_ready();
        Response::immediate(self, ready)
        //let val = self.ready.is_high().unwrap_or(false);
        //Response::immediate(self, val)
    }
}

impl RequestHandler<AwaitReady> for EsWifiReadyPin {
    type Response = ();

    fn on_request(mut self, message: AwaitReady) -> Response<Self, Self::Response> {
        //if self.ready.is_high().unwrap_or(false) {
        //self.shared.unwrap().signal_ready(true);
        //Response::immediate(self, ())
        //} else {
        let future = AwaitReadyFuture::new(self.shared.unwrap());
        Response::immediate_future(self, future)
        //}
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
                if self.shared.set_waker(cx.waker().clone()) {
                    return Poll::Ready(());
                }
                self.waiting = true;
            }
            Poll::Pending
        }
    }
}
