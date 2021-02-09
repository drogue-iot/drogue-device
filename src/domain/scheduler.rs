use crate::domain::time::duration::{Duration, Milliseconds};
use crate::prelude::{Actor, Address, NotifyHandler};

#[derive(Clone)]
pub struct Schedule<A, DUR, E>
where
    A: Actor + NotifyHandler<E> + 'static,
    DUR: Duration + Into<Milliseconds>,
    E: Clone + 'static,
{
    pub delay: DUR,
    pub event: E,
    pub address: Address<A>,
}

pub trait Scheduler: Actor {
    fn schedule<A, DUR, E>(&mut self, schedule: Schedule<A, DUR, E>)
    where
        A: Actor + NotifyHandler<E> + 'static,
        DUR: Duration + Into<Milliseconds> + 'static,
        E: Clone + 'static;
}

impl<S: Scheduler> Address<S> {
    pub fn schedule<DUR, E, A>(&self, delay: DUR, event: E, address: Address<A>)
    where
        DUR: Duration + Into<Milliseconds> + 'static,
        E: Clone + 'static,
        A: Actor + NotifyHandler<E>,
    {
        self.notify(Schedule::new(delay, event, address));
    }
}
