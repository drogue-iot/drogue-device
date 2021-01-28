

use crate::domain::time::duration::{Duration, Milliseconds};
use crate::hal::timer::Timer as HalTimer;
use crate::prelude::*;
use core::cell::UnsafeCell;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};



#[derive(Copy, Clone, Debug)]
pub struct Delay<DUR: Duration + Into<Milliseconds>>(pub DUR);

#[derive(Clone)]
pub struct Schedule<D: Device, A: Actor<D>, DUR: Duration + Into<Milliseconds>, E> {
    delay: DUR,
    event: E,
    address: Address<D, A>,
}

impl<D: Device, A: Actor<D>, DUR: Duration + Into<Milliseconds>, E> Schedule<D, A, DUR, E> {
    pub fn new(delay: DUR, event: E, address: Address<D, A>) -> Self {
        Self {
            delay,
            event,
            address,
        }
    }
}

pub struct Timer<D: Device, T: HalTimer> {
    timer: T,
    current_delay_deadline: Option<Milliseconds>,
    delay_deadlines: [Option<DelayDeadline>; 16],
    _device: PhantomData<D>,
}

impl<D: Device, T: HalTimer> Timer<D, T> {
    pub fn new(timer: T) -> Self {
        Self {
            timer,
            current_delay_deadline: None,
            delay_deadlines: Default::default(),
            _device: PhantomData,
        }
    }

    fn has_expired(&mut self, index: usize) -> bool {
        let expired =
            self.delay_deadlines[index].as_ref().unwrap().expiration == Milliseconds(0u32);
        if expired {
            self.delay_deadlines[index].take();
        }

        expired
    }

    fn register_waker(&mut self, index: usize, waker: Waker) {
        log::info!("Registering waker");
        self.delay_deadlines[index]
            .as_mut()
            .unwrap()
            .waker
            .replace(waker);
    }

    fn configure_timer<R, F: FnOnce(&mut Timer<D, T>, Option<usize>) -> R>(
        &mut self,
        ms: Milliseconds,
        f: F,
    ) -> R {
        if let Some((index, slot)) = self
            .delay_deadlines
            .iter_mut()
            .enumerate()
            .find(|e| matches!(e, (_, None)))
        {
            self.delay_deadlines[index].replace(DelayDeadline::new(ms));
            if let Some(current_deadline) = self.current_delay_deadline {
                if current_deadline > ms {
                    self.current_delay_deadline.replace(ms);
                    //log::info!("start shorter timer for {:?}", ms);
                    self.timer.start(ms);
                } else {
                    //log::info!("timer already running for {:?}", current_deadline );
                }
            } else {
                self.current_delay_deadline.replace(ms);
                //log::info!("start new timer for {:?}", ms);
                self.timer.start(ms);
            }
            (f)(self, Some(index))
        } else {
            (f)(self, None)
        }
    }
}

impl<D: Device, T: HalTimer> Actor<D> for Timer<D, T> {}

impl<D: Device, T: HalTimer> NotificationHandler<Lifecycle> for Timer<D, T> {
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D: Device, T: HalTimer, DUR: Duration + Into<Milliseconds>>
    RequestHandler<D, Delay<DUR>> for Timer<D, T>
{
    type Response = ();

    fn on_request(&'static mut self, message: Delay<DUR>) -> Response<Self::Response> {
        let ms: Milliseconds = message.0.into();
        //log::info!("delay request {:?}", ms);

        self.configure_timer(ms, |timer, index| {
            if let Some(index) = index {
                Response::immediate_future(DelayFuture::new(index, timer))
            } else {
                Response::immediate(())
            }
        })
    }
}

impl<
        D: Device,
        T: HalTimer,
        E: 'static,
        A: Actor<D> + NotificationHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds> + 'static,
    > NotificationHandler<Schedule<D, A, DUR, E>> for Timer<D, T>
{
    fn on_notification(&'static mut self, message: Schedule<D, A, DUR, E>) -> Completion {
        let ms: Milliseconds = message.delay.into();
        self.configure_timer(ms, |timer, index| {
            if let Some(index) = index {
                let f = DelayFuture::new(index, timer);
                Completion::defer(async move {
                    log::info!("Awaiting future");
                    f.await;
                    log::info!("NOTIFYING");
                    message.address.notify(message.event);
                })
            } else {
                Completion::immediate()
            }
        })
    }
}

impl<D: Device, T: HalTimer> Interrupt<D> for Timer<D, T> {
    fn on_interrupt(&mut self) {
        self.timer.clear_update_interrupt_flag();
        let expired = self.current_delay_deadline.unwrap();

        let mut next_deadline = None;
        //log::info!("timer expired! {:?}", expired);
        for slot in self.delay_deadlines.iter_mut() {
            if let Some(deadline) = slot {
                if deadline.expiration >= expired {
                    deadline.expiration = deadline.expiration - expired;
                } else {
                    deadline.expiration = Milliseconds(0u32);
                }

                if deadline.expiration == Milliseconds(0u32) {
                    deadline.waker.take().unwrap().wake();
                } else {
                    match next_deadline {
                        None => {
                            next_deadline.replace(deadline.expiration);
                        }
                        Some(soonest) if soonest > deadline.expiration => {
                            next_deadline.replace(deadline.expiration);
                        }
                        _ => { /* ignore */ }
                    }
                }
            }
        }

        //log::info!("next deadline {:?}", next_deadline );

        if let Some(next_deadline) = next_deadline {
            if next_deadline > Milliseconds(0u32) {
                self.current_delay_deadline.replace(next_deadline);
                self.timer.start(next_deadline);
            } else {
                self.current_delay_deadline.take();
            }
        } else {
            self.current_delay_deadline.take();
        }
    }
}

impl<D: Device + 'static, T: HalTimer+ 'static>
    Address<D, Timer<D, T>>
{
    pub async fn delay<DUR: Duration + Into<Milliseconds> + 'static>(&self, duration: DUR) {
        self.request(Delay(duration)).await
    }

    pub fn schedule<
        DUR: Duration + Into<Milliseconds> + 'static,
        E: 'static,
        A: Actor<D> + NotificationHandler<E> + 'static,
    >(
        &self,
        delay: DUR,
        event: E,
        address: Address<D, A>,
    ) {
        self.notify(Schedule::new(delay, event, address));
    }
}

struct DelayDeadline {
    expiration: Milliseconds,
    waker: Option<Waker>,
}

impl DelayDeadline {
    fn new(expiration: Milliseconds) -> Self {
        Self {
            expiration,
            waker: None,
        }
    }
}

struct DelayFuture<D: Device, T: HalTimer> {
    index: usize,
    timer: UnsafeCell<*mut Timer<D, T>>,
    expired: bool,
}

impl<D: Device, T: HalTimer> DelayFuture<D, T> {
    fn new(index: usize, timer: &mut Timer<D, T>) -> Self {
        Self {
            index,
            timer: UnsafeCell::new(timer),
            expired: false,
        }
    }

    fn has_expired(&mut self) -> bool {
        if !self.expired {
            self.expired = unsafe {
                // critical section to avoid being trampled by the timer's own IRQ
                cortex_m::interrupt::free(|cs| (&mut **self.timer.get()).has_expired(self.index))
            }
        }

        self.expired
    }

    fn register_waker(&self, waker: &Waker) {
        unsafe {
            (&mut **self.timer.get()).register_waker(self.index, waker.clone());
        }
    }
}

impl<D: Device, T: HalTimer> Future for DelayFuture<D, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.has_expired() {
            //log::info!("delay poll - ready {}", self.index);
            Poll::Ready(())
        } else {
            //log::info!("delay poll - pending {}", self.index);
            self.register_waker(cx.waker());
            Poll::Pending
        }
    }
}
