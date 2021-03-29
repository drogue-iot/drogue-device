use crate::arena::{Arena, Box};
use crate::domain::time::duration::{Duration, Milliseconds};
use crate::prelude::*;
//use crate::system::SystemArena;

pub trait Timer: Actor<Request = TimerRequest, Response = ()> {
    /*
    pub async fn delay<DUR>(&self, delay: DUR)
    where
        DUR: Duration + Into<Milliseconds>;


    pub fn schedule<A, DUR, E>(&self, delay: DUR, event: E, address: Address<A>)
    where
        A: Actor<Request = E>,
        DUR: Duration + Into<Milliseconds>,
        E: 'static;*/
}

pub trait Schedulable {
    fn run(&mut self);
}

pub enum TimerRequest {
    Schedule(Milliseconds, Box<dyn Schedulable, SystemArena>),
    Delay(Milliseconds),
}

impl<T> Address<T>
where
    T: Timer,
{
    pub async fn delay<DUR>(&self, delay: DUR)
    where
        DUR: Duration + Into<Milliseconds>,
    {
        self.request(TimerRequest::Delay(delay.into())).await
    }

    pub fn schedule<A, DUR, E>(&self, delay: DUR, event: E, address: Address<A>)
    where
        A: Actor<Request = E>,
        DUR: Duration + Into<Milliseconds>,
        E: 'static,
    {
        let scheduled = Scheduled::new(address, event);
        let scheduled: Box<dyn Schedulable, SystemArena> =
            Box::new(SystemArena::alloc(scheduled).unwrap());
        self.notify(TimerRequest::Schedule(delay.into(), scheduled));
    }
}

struct Scheduled<A, E>
where
    A: Actor<Request = E> + 'static,
    E: 'static,
{
    dest: Address<A>,
    event: Option<E>,
}

impl<A, E> Schedulable for Scheduled<A, E>
where
    A: Actor<Request = E> + 'static,
    E: 'static,
{
    fn run(&mut self) {
        self.dest.notify(self.event.take().unwrap());
    }
}

impl<A, E> Scheduled<A, E>
where
    A: Actor<Request = E>,
    E: 'static,
{
    fn new(dest: Address<A>, event: E) -> Self {
        Self {
            dest,
            event: Some(event),
        }
    }
}
