// use crate::arch::with_critical_section;
// use crate::hal::gpio::InterruptPin;
// use crate::prelude::*;
// use core::cell::RefCell;
// use core::future::Future;
// use core::pin::Pin;
// use core::sync::atomic::{AtomicBool, Ordering};
// use core::task::{Context, Poll, Waker};
// use cortex_m::interrupt::Nr;
// use embedded_hal::digital::v2::InputPin;

// pub struct Shared {
//     ready: AtomicBool,
//     ready_waker: RefCell<Option<Waker>>,
// }

// impl Shared {
//     fn new() -> Self {
//         Self {
//             ready: AtomicBool::new(false),
//             ready_waker: RefCell::new(None),
//         }
//     }

//     fn is_ready(&self) -> bool {
//         self.ready.load(Ordering::Acquire)
//     }

//     fn poll_ready(&self, waker: &Waker) -> Poll<()> {
//         with_critical_section(|cs| {
//             let ready = self.ready.load(Ordering::Acquire);
//             if ready {
//                 self.ready_waker.borrow_mut().take();
//                 Poll::Ready(())
//             } else {
//                 self.ready_waker.borrow_mut().replace(waker.clone());
//                 Poll::Pending
//             }
//         })
//     }

//     fn signal_ready(&self, ready: bool) {
//         self.ready.store(ready, Ordering::Release);
//         if ready {
//             if let Some(waker) = self.ready_waker.borrow_mut().take() {
//                 waker.wake()
//             }
//         }
//     }
// }

// pub struct EsWifiReady<READY>
// where
//     READY: InputPin + InterruptPin + 'static,
// {
//     shared: Shared,
//     ready: ActorContext<EsWifiReadyPin>,
//     irq: InterruptContext<EsWifiReadyInterrupt<READY>>,
// }

// impl<READY> EsWifiReady<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     pub fn new<IRQ: Nr>(ready: READY, irq: IRQ) -> Self {
//         Self {
//             shared: Shared::new(),
//             ready: ActorContext::new(EsWifiReadyPin::new()),
//             irq: InterruptContext::new(EsWifiReadyInterrupt::new(ready), irq),
//         }
//     }
// }

// impl<READY> Package for EsWifiReady<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     type Primary = EsWifiReadyPin;
//     type Configuration = ();

//     fn mount(
//         &'static self,
//         config: Self::Configuration,
//         supervisor: &mut Supervisor,
//     ) -> Address<Self::Primary> {
//         let addr = self.ready.mount(&self.shared, supervisor);
//         self.irq.mount(&self.shared, supervisor);
//         addr
//     }

//     fn primary(&'static self) -> Address<Self::Primary> {
//         self.ready.address()
//     }
// }

// pub struct EsWifiReadyPin {
//     shared: Option<&'static Shared>,
// }

// impl EsWifiReadyPin {
//     fn new() -> Self {
//         Self { shared: None }
//     }
// }

// impl Actor for EsWifiReadyPin {
//     type Configuration = &'static Shared;

//     fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
//     where
//         Self: Sized,
//     {
//         self.shared.replace(config);
//     }
// }

// pub struct EsWifiReadyInterrupt<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     ready: READY,
//     shared: Option<&'static Shared>,
// }

// impl<READY> EsWifiReadyInterrupt<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     pub fn new(ready: READY) -> Self {
//         Self {
//             ready,
//             shared: None,
//         }
//     }
// }

// impl<READY> Actor for EsWifiReadyInterrupt<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     type Configuration = &'static Shared;

//     fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
//     where
//         Self: Sized,
//     {
//         self.shared.replace(config);
//     }
// }

// impl<READY> Interrupt for EsWifiReadyInterrupt<READY>
// where
//     READY: InputPin + InterruptPin,
// {
//     fn on_interrupt(&mut self) {
//         if self.ready.is_high().unwrap_or(false) {
//             self.shared.unwrap().signal_ready(true);
//         } else {
//             self.shared.unwrap().signal_ready(false);
//         }
//         self.ready.clear_interrupt();
//     }
// }

// pub struct AwaitReady;
// pub struct QueryReady;

// impl RequestHandler<QueryReady> for EsWifiReadyPin {
//     type Response = bool;

//     fn on_request(self, _message: QueryReady) -> Response<Self, Self::Response> {
//         let ready = self.shared.unwrap().is_ready();
//         Response::immediate(self, ready)
//     }
// }

// impl RequestHandler<AwaitReady> for EsWifiReadyPin {
//     type Response = ();

//     fn on_request(self, message: AwaitReady) -> Response<Self, Self::Response> {
//         if self.shared.unwrap().is_ready() {
//             Response::immediate(self, ())
//         } else {
//             let future = AwaitReadyFuture::new(self.shared.unwrap());
//             Response::immediate_future(self, future)
//         }
//     }
// }

// struct AwaitReadyFuture {
//     shared: &'static Shared,
// }

// impl AwaitReadyFuture {
//     fn new(shared: &'static Shared) -> Self {
//         Self { shared }
//     }
// }

// impl Future for AwaitReadyFuture {
//     type Output = ();

//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         self.shared.poll_ready(cx.waker())
//     }
// }
