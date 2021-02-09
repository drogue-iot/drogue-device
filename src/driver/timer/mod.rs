use crate::alloc::{alloc, Box};
use crate::domain::delayer::{Delay, Delayer};
use crate::domain::scheduler::{Schedule, Scheduler};
use crate::domain::time::duration::{Duration, Milliseconds};
use crate::hal::timer::Timer as HalTimer;
use crate::prelude::*;
use core::cell::RefCell;
use core::fmt::{Debug, Formatter};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;

impl<DUR: Duration + Into<Milliseconds>> Debug for Delay<DUR> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Delay")
    }
}

pub trait Schedulable {
    fn run(&self);
    fn get_expiration(&self) -> Milliseconds;
    fn set_expiration(&mut self, expiration: Milliseconds);
}

pub struct Shared {
    current_deadline: RefCell<Option<Milliseconds>>,
    delay_deadlines: RefCell<[Option<DelayDeadline>; 16]>,
    schedule_deadlines: RefCell<[Option<Box<dyn Schedulable>>; 16]>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            current_deadline: RefCell::new(None),
            delay_deadlines: RefCell::new(Default::default()),
            schedule_deadlines: RefCell::new(Default::default()),
        }
    }

    fn has_expired(&self, index: usize) -> bool {
        let expired = self.delay_deadlines.borrow()[index]
            .as_ref()
            .unwrap()
            .expiration
            == Milliseconds(0u32);
        if expired {
            self.delay_deadlines.borrow_mut()[index].take();
        }
        expired
    }

    fn register_waker(&self, index: usize, waker: Waker) {
        self.delay_deadlines.borrow_mut()[index]
            .as_mut()
            .unwrap()
            .waker
            .replace(waker);
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Timer<T: HalTimer + 'static> {
    actor: InterruptContext<TimerActor<T>>,
    shared: Shared,
}

impl<T: HalTimer> Timer<T> {
    pub fn new<IRQ: Nr>(timer: T, irq: IRQ) -> Self {
        Self {
            actor: InterruptContext::new(TimerActor::new(timer), irq).with_name("timer"),
            shared: Shared::default(),
        }
    }
}

impl<T: HalTimer> Package for Timer<T> {
    type Primary = TimerActor<T>;
    type Configuration = ();

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        self.actor.mount(&self.shared, supervisor)
    }
}

pub struct TimerActor<T: HalTimer> {
    timer: T,
    shared: Option<&'static Shared>,
}

impl<T: HalTimer> TimerActor<T> {
    pub fn new(timer: T) -> Self {
        Self {
            timer,
            shared: None,
        }
    }
}

impl<T: HalTimer> Scheduler for TimerActor<T> {
    fn schedule<A, DUR, E>(&mut self, message: Schedule<A, DUR, E>)
    where
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds> + 'static,
        E: Clone + 'static,
    {
        let ms: Milliseconds = message.delay.into();
        // log::info!("schedule request {:?}", ms);
        let mut deadlines = self.shared.unwrap().schedule_deadlines.borrow_mut();
        let mut current_deadline = self.shared.unwrap().current_deadline.borrow_mut();

        if let Some((index, slot)) = deadlines
            .iter_mut()
            .enumerate()
            .find(|e| matches!(e, (_, None)))
        {
            deadlines[index].replace(Box::new(alloc(ScheduleDeadline::new(ms, message)).unwrap()));
            if let Some(current) = &*current_deadline {
                if *current > ms {
                    current_deadline.replace(ms);
                    self.timer.start(ms);
                } else {
                    //log::info!("timer already running for {:?}", current_deadline );
                }
            } else {
                current_deadline.replace(ms);
                //log::info!("start new timer for {:?}", ms);
                self.timer.start(ms);
            }
        }
    }
}

impl<T: HalTimer> Delayer for TimerActor<T> {
    fn delay<DUR>(mut self, message: Delay<DUR>) -> Response<Self, ()>
    where
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        let ms: Milliseconds = message.0.into();

        let mut delay_deadlines = self.shared.unwrap().delay_deadlines.borrow_mut();
        if let Some((index, slot)) = delay_deadlines
            .iter_mut()
            .enumerate()
            .find(|e| matches!(e, (_, None)))
        {
            delay_deadlines[index].replace(DelayDeadline::new(ms));
            let mut current_deadline = self.shared.unwrap().current_deadline.borrow_mut();
            let (new_deadline, should_replace) = if let Some(current_deadline) = *current_deadline {
                if current_deadline > ms {
                    //log::info!("start shorter timer for {:?}", ms);
                    (ms, true)
                } else {
                    //log::info!("timer already running for {:?}", current_deadline );
                    (ms, false)
                }
            } else {
                //log::info!("start new timer for {:?}", ms);
                (ms, true)
            };

            if should_replace {
                current_deadline.replace(new_deadline);
                self.timer.start(new_deadline);
            }
            let future = DelayFuture::new(index, self.shared.as_ref().unwrap());
            Response::immediate_future(self, future)
        } else {
            Response::immediate(self, ())
        }
    }
}

impl<T: HalTimer> Actor for TimerActor<T> {
    type Configuration = &'static Shared;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.shared.replace(config);
    }
}

impl<T: HalTimer> Interrupt for TimerActor<T> {
    fn on_interrupt(&mut self) {
        self.timer.clear_update_interrupt_flag();
        let expired = self.shared.unwrap().current_deadline.borrow().unwrap();

        let mut delay_deadlines = self.shared.unwrap().delay_deadlines.borrow_mut();

        let mut next_deadline = None;
        //log::info!("timer expired! {:?}", expired);
        for slot in delay_deadlines.iter_mut() {
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

        let mut schedule_deadlines = self.shared.unwrap().schedule_deadlines.borrow_mut();

        for slot in schedule_deadlines.iter_mut() {
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

        let mut current_deadline = self.shared.unwrap().current_deadline.borrow_mut();
        //log::info!("next deadline {:?}", next_deadline );

        if let Some(next_deadline) = next_deadline {
            if next_deadline > Milliseconds(0u32) {
                current_deadline.replace(next_deadline);
                self.timer.start(next_deadline);
            } else {
                current_deadline.take();
            }
        } else {
            current_deadline.take();
        }
    }
}

impl<T: HalTimer + 'static> Address<TimerActor<T>> {
    /*
    pub async fn delay<DUR: Duration + Into<Milliseconds> + 'static>(&self, duration: DUR) {
        self.request(Delay(duration)).await
    }

     */

    /*
    pub fn schedule<
        DUR: Duration + Into<Milliseconds> + 'static,
        E: Clone + 'static,
        A: Actor + NotifyHandler<E>,
    >(
        &self,
        delay: DUR,
        event: E,
        address: Address<A>,
    ) {
        self.notify(Schedule::new(delay, event, address));
    }

     */
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

struct DelayFuture {
    index: usize,
    shared: &'static Shared,
    expired: bool,
}

impl DelayFuture {
    fn new(index: usize, shared: &'static Shared) -> Self {
        Self {
            index,
            shared,
            expired: false,
        }
    }

    fn has_expired(&mut self) -> bool {
        if !self.expired {
            // critical section to avoid being trampled by the timer's own IRQ
            self.expired = cortex_m::interrupt::free(|cs| self.shared.has_expired(self.index))
        }

        self.expired
    }

    fn register_waker(&self, waker: &Waker) {
        //unsafe {
        //(&mut **self.timer.get()).register_waker(self.index, waker.clone());
        //}
        self.shared.register_waker(self.index, waker.clone());
    }
}

impl Future for DelayFuture {
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
