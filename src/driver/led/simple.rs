use crate::prelude::*;
use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;
use crate::hal::Active;

pub struct On;

pub struct Off;

pub trait Switchable<D: Device>: Actor<D> + NotificationHandler<On> + NotificationHandler<Off> {
    fn turn_on(&mut self);
    fn turn_off(&mut self);
}

pub struct SimpleLED<D: Device, P: OutputPin> {
    active: Active,
    pin: P,
    _marker: PhantomData<D>,
}


impl<D: Device, P: OutputPin> SimpleLED<D, P> {
    pub fn new(pin: P, active: Active) -> Self {
        Self {
            active,
            pin,
            _marker: PhantomData,
        }
    }
}

impl<D: Device, P: OutputPin> Switchable<D> for SimpleLED<D, P> {
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

impl<D: Device, P: OutputPin> Actor<D> for SimpleLED<D, P> {}

impl<D: Device, P: OutputPin> NotificationHandler<Lifecycle> for SimpleLED<D, P>
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D: Device, P: OutputPin> NotificationHandler<On> for SimpleLED<D, P>
    where
        Self: Switchable<D>,
{
    fn on_notification(&'static mut self, message: On) -> Completion {
        self.turn_on();
        Completion::immediate()
    }
}

impl<D: Device, P: OutputPin> NotificationHandler<Off> for SimpleLED<D, P>
    where
        Self: Switchable<D>,
{
    fn on_notification(&'static mut self, message: Off) -> Completion {
        Completion::defer(async move {
            self.turn_off();
        })
    }
}

impl<D: Device + 'static, S> Address<D, S>
    where
        S: Actor<D> + 'static,
        S: Switchable<D>,
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
