use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use core::marker::PhantomData;
use crate::driver::{Active, ActiveHigh, ActiveLow};

pub struct On;

pub struct Off;

pub trait Switchable {
    fn turn_on(&mut self);
    fn turn_off(&mut self);
}

pub struct SimpleLED<P: OutputPin, A: Active> {
    pin: P,
    _active: PhantomData<A>,
}

impl<P: OutputPin, A: Active> SimpleLED<P, A> {
    pub fn into_active_low(self) -> SimpleLED<P, ActiveLow> {
        SimpleLED {
            pin: self.pin,
            _active: PhantomData
        }
    }

    pub fn into_active_high(self) -> SimpleLED<P, ActiveHigh> {
        SimpleLED {
            pin: self.pin,
            _active: PhantomData
        }
    }

}

impl<P: OutputPin> SimpleLED<P, ActiveHigh> {
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            _active: PhantomData,
        }
    }
}

impl<P: OutputPin> Switchable for SimpleLED<P, ActiveHigh> {
    fn turn_on(&mut self) {
        self.pin.set_high().ok().unwrap();
    }

    fn turn_off(&mut self) {
        self.pin.set_low().ok().unwrap();
    }
}

impl<P: OutputPin> Switchable for SimpleLED<P, ActiveLow> {
    fn turn_on(&mut self) {
        self.pin.set_low().ok().unwrap();
    }

    fn turn_off(&mut self) {
        self.pin.set_high().ok().unwrap();
    }
}

impl<P: OutputPin, A: Active> Actor for SimpleLED<P, A> {}

impl<P: OutputPin, A: Active> NotificationHandler<On> for SimpleLED<P, A>
    where Self: Switchable
{
    fn on_notification(&'static mut self, message: On) -> Completion {
        self.turn_on();
        Completion::immediate()
    }
}

impl<P: OutputPin, A: Active> NotificationHandler<Off> for SimpleLED<P, A>
    where Self: Switchable
{
    fn on_notification(&'static mut self, message: Off) -> Completion {
        Completion::defer(async move {
            self.turn_off();
        })
    }
}

impl<S> Address<S>
    where S: Actor + 'static,
          S: Switchable,
          S: NotificationHandler<On>,
          S: NotificationHandler<Off>,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }

    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
