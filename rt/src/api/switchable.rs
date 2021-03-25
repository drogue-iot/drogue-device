use crate::prelude::{Actor, Address, RequestHandler};

pub struct On;

pub struct Off;

pub trait Switchable: Actor + RequestHandler<On> + RequestHandler<Off> {}

impl<S> Address<S>
where
    S: RequestHandler<On>,
    S: Actor + 'static,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }
}

impl<S> Address<S>
where
    S: RequestHandler<Off>,
    S: Actor + 'static,
{
    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
