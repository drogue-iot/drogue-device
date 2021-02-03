use crate::hal::gpio::ActiveOutput;
use crate::hal::Active;
use crate::prelude::*;
use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;

pub struct On;

pub struct Off;

pub trait Switchable: Actor + NotifyHandler<On> + NotifyHandler<Off> {
    fn turn_on(&mut self);
    fn turn_off(&mut self);
}

pub struct SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    pin: P,
    _active: PhantomData<A>,
}

impl<P, A> SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    pub fn new(pin: P, active: Active) -> Self {
        Self {
            pin,
            _active: PhantomData,
        }
    }
}

impl<P, A> Switchable for SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    fn turn_on(&mut self) {
        A::set_active(&mut self.pin);
    }

    fn turn_off(&mut self) {
        A::set_inactive(&mut self.pin);
    }
}

impl<P, A> Actor for SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
}

impl<P,A> NotifyHandler<On> for SimpleLED<P,A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    fn on_notify(&'static mut self, message: On) -> Completion<Self> {
        self.turn_on();
        Completion::immediate(self)
    }
}

impl<P,A> NotifyHandler<Off> for SimpleLED<P,A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    fn on_notify(&'static mut self, message: Off) -> Completion<Self> {
        Completion::defer(async move {
            self.turn_off();
            (self)
        })
    }
}

impl<S> Address<S>
where
    S: NotifyHandler<Off> + NotifyHandler<On>,
    S: Actor + 'static,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }

    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
