use crate::prelude::{Actor, NotifyHandler, Address};

pub struct On;

pub struct Off;

pub trait Switchable: Actor + NotifyHandler<On> + NotifyHandler<Off> {

}

impl<S> Address<S>
    where
        S: NotifyHandler<On>,
        S: Actor + 'static,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }
}

impl<S> Address<S>
    where
        S: NotifyHandler<Off>,
        S: Actor + 'static,
{
    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
