use crate::prelude::{Actor, Address};

pub enum Switch {
    On,
    Off,
}

pub trait Switchable: Actor<Request = Switch> {}

impl<S> Address<S>
where
    S: Actor<Request = Switch> + 'static,
{
    pub fn turn_on(&self) {
        self.notify(Switch::On);
    }
}

impl<S> Address<S>
where
    S: Actor<Request = Switch> + 'static,
{
    pub fn turn_off(&self) {
        self.notify(Switch::Off);
    }
}
