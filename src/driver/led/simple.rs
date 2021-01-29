use crate::prelude::*;
use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;
use crate::hal::Active;

pub struct On;

pub struct Off;

pub trait Switchable: Actor + NotifyHandler<On> + NotifyHandler<Off>
{
    fn turn_on(&mut self);
    fn turn_off(&mut self);
}

pub struct SimpleLED<P>
    where
        P: OutputPin
{
    active: Active,
    pin: P,
}


impl<P> SimpleLED<P>
    where
        P: OutputPin
{
    pub fn new(pin: P, active: Active) -> Self {
        Self {
            active,
            pin,
        }
    }
}

impl<P> Switchable for SimpleLED<P>
    where
        P: OutputPin
{
    fn turn_on(&mut self) {
        match self.active {
            Active::High => {
                self.pin.set_high().ok().unwrap();
            }
            Active::Low => {
                self.pin.set_low().ok().unwrap();
            }
        }
    }

    fn turn_off(&mut self) {
        match self.active {
            Active::High => {
                self.pin.set_low().ok().unwrap();
            }
            Active::Low => {
                self.pin.set_high().ok().unwrap();
            }
        }
    }
}

impl<P> Actor for SimpleLED<P>
    where
        P: OutputPin {}

impl<P> NotifyHandler<On> for SimpleLED<P>
    where
        Self: Switchable,
        P: OutputPin
{
    fn on_notify(&'static mut self, message: On) -> Completion {
        self.turn_on();
        Completion::immediate()
    }
}

impl<P> NotifyHandler<Off> for SimpleLED<P>
    where
        Self: Switchable,
        P: OutputPin
{
    fn on_notify(&'static mut self, message: Off) -> Completion {
        Completion::defer(async move {
            self.turn_off();
        })
    }
}

impl<S> Address<S>
    where
        S: NotifyHandler<Off>,
        S: Actor + 'static,
        S: Switchable,
        S: NotifyHandler<On>,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }

    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
