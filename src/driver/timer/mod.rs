use crate::alloc::{alloc, Box};
use crate::domain::time::duration::{Duration, Milliseconds};
use crate::hal::timer::Timer as HalTimer;
use crate::prelude::*;
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

#[derive(Copy, Clone, Debug)]
pub struct Delay<DUR: Duration + Into<Milliseconds>>(pub DUR);

pub trait Schedulable {
    fn run(&self);
    fn get_expiration(&self) -> Milliseconds;
    fn set_expiration(&mut self, expiration: Milliseconds);
}

#[derive(Clone)]
pub struct Schedule<
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds>,
    E: Clone + 'static,
> {
    delay: DUR,
    event: E,
    address: Address<A>,
}

impl<
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds>,
        E: Clone + 'static,
    > Schedule<A, DUR, E>
{
    pub fn new(delay: DUR, event: E, address: Address<A>) -> Self {
        Self {
            delay,
            event,
            address,
        }
    }
}

pub struct Timer<T: HalTimer> {
    timer: T,
    current_deadline: Option<Milliseconds>,
    delay_deadlines: [Option<DelayDeadline>; 16],
    schedule_deadlines: [Option<Box<dyn Schedulable>>; 16],
}

impl<T: HalTimer> Timer<T> {
    pub fn new(timer: T) -> Self {
        Self {
            timer,
            current_deadline: None,
            delay_deadlines: Default::default(),
            schedule_deadlines: Default::default(),
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
        // log::info!("Registering waker");
        self.delay_deadlines[index]
            .as_mut()
            .unwrap()
            .waker
            .replace(waker);
    }
}

impl<T: HalTimer> Actor for Timer<T> {}

impl<T: HalTimer, DUR: Duration + Into<Milliseconds>> RequestHandler<Delay<DUR>> for Timer<T> {
    type Response = ();

    fn on_request(&'static mut self, message: Delay<DUR>) -> Response<Self::Response> {
        let ms: Milliseconds = message.0.into();
        //log::info!("delay request {:?}", ms);

        if let Some((index, slot)) = self
            .delay_deadlines
            .iter_mut()
            .enumerate()
            .find(|e| matches!(e, (_, None)))
        {
            self.delay_deadlines[index].replace(DelayDeadline::new(ms));
            if let Some(current_deadline) = self.current_deadline {
                if current_deadline > ms {
                    self.current_deadline.replace(ms);
                    //log::info!("start shorter timer for {:?}", ms);
                    self.timer.start(ms);
                } else {
                    //log::info!("timer already running for {:?}", current_deadline );
                }
            } else {
                self.current_deadline.replace(ms);
                //log::info!("start new timer for {:?}", ms);
                self.timer.start(ms);
            }
            Response::immediate_future(DelayFuture::new(index, self))
        } else {
            Response::immediate(())
        }
    }
}

impl<
        T: HalTimer,
        E: Clone + 'static,
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds> + 'static,
    > NotifyHandler<Schedule<A, DUR, E>> for Timer<T>
{
    fn on_notify(&'static mut self, message: Schedule<A, DUR, E>) -> Completion {
        let ms: Milliseconds = message.delay.into();
        //log::info!("delay request {:?}", ms);

        if let Some((index, slot)) = self
            .schedule_deadlines
            .iter_mut()
            .enumerate()
            .find(|e| matches!(e, (_, None)))
        {
            self.schedule_deadlines[index]
                .replace(Box::new(alloc(ScheduleDeadline::new(ms, message)).unwrap()));
            if let Some(current_deadline) = self.current_deadline {
                if current_deadline > ms {
                    self.current_deadline.replace(ms);
                    //log::info!("start shorter timer for {:?}", ms);
                    self.timer.start(ms);
                } else {
                    //log::info!("timer already running for {:?}", current_deadline );
                }
            } else {
                self.current_deadline.replace(ms);
                //log::info!("start new timer for {:?}", ms);
                self.timer.start(ms);
            }
        }
        Completion::immediate()
    }
}

impl<T: HalTimer> Interrupt for Timer<T> {
    fn on_interrupt(&mut self) {
        self.timer.clear_update_interrupt_flag();
        let expired = self.current_deadline.unwrap();

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

        for slot in self.schedule_deadlines.iter_mut() {
            if let Some(deadline) = slot {
                let expiration = deadline.get_expiration();
                if expiration >= expired {
                    deadline.set_expiration(expiration - expired);
                } else {
                    deadline.set_expiration(Milliseconds(0u32));
                }

                if deadline.get_expiration() == Milliseconds(0u32) {
                    deadline.run();
                    slot.take();
                } else {
                    match next_deadline {
                        None => {
                            next_deadline.replace(deadline.get_expiration());
                        }
                        Some(soonest) if soonest > deadline.get_expiration() => {
                            next_deadline.replace(deadline.get_expiration());
                        }
                        _ => { /* ignore */ }
                    }
                }
            }
        }

        //log::info!("next deadline {:?}", next_deadline );

        if let Some(next_deadline) = next_deadline {
            if next_deadline > Milliseconds(0u32) {
                self.current_deadline.replace(next_deadline);
                self.timer.start(next_deadline);
            } else {
                self.current_deadline.take();
            }
        } else {
            self.current_deadline.take();
        }
    }
}

impl<T: HalTimer + 'static> Address<Timer<T>> {
    pub async fn delay<DUR: Duration + Into<Milliseconds> + 'static>(&self, duration: DUR) {
        self.request(Delay(duration)).await
    }

    pub fn schedule<
        DUR: Duration + Into<Milliseconds> + 'static,
        E: Clone + 'static,
        A: Actor + NotifyHandler<E> + 'static,
    >(
        &self,
        delay: DUR,
        event: E,
        address: Address<A>,
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

pub struct ScheduleDeadline<
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds>,
    E: Clone + 'static,
> {
    expiration: Milliseconds,
    schedule: Schedule<A, DUR, E>,
}

impl<
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds>,
        E: Clone + 'static,
    > Schedulable for ScheduleDeadline<A, DUR, E>
{
    fn run(&self) {
        self.schedule.address.notify(self.schedule.event.clone());
    }

    fn set_expiration(&mut self, expiration: Milliseconds) {
        self.expiration = expiration;
    }

    fn get_expiration(&self) -> Milliseconds {
        self.expiration
    }
}

impl<
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds>,
        E: Clone + 'static,
    > ScheduleDeadline<A, DUR, E>
{
    fn new(expiration: Milliseconds, schedule: Schedule<A, DUR, E>) -> Self {
        Self {
            expiration,
            schedule,
        }
    }
}

struct DelayFuture<T: HalTimer> {
    index: usize,
    timer: UnsafeCell<*mut Timer<T>>,
    expired: bool,
}

impl<T: HalTimer> DelayFuture<T> {
    fn new(index: usize, timer: &mut Timer<T>) -> Self {
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

impl<T: HalTimer> Future for DelayFuture<T> {
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
