use crate::prelude::*;
use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;
use crate::hal::Active;

pub struct On;

pub struct Off;

pub trait Switchable<D>: Actor<D> + NotificationHandler<On> + NotificationHandler<Off>
    where D: Device
{
    fn turn_on(&mut self);
    fn turn_off(&mut self);
}

pub struct SimpleLED<D, P>
    where D: Device, P: OutputPin
{
    active: Active,
    pin: P,
    _marker: PhantomData<D>,
}


impl<D, P> SimpleLED<D, P>
    where D: Device, P: OutputPin
{
    pub fn new(pin: P, active: Active) -> Self {
        Self {
            active,
            pin,
            _marker: PhantomData,
        }
    }
}

impl<D, P> Switchable<D> for SimpleLED<D, P>
    where D: Device, P: OutputPin
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

impl<D, P> Actor<D> for SimpleLED<D, P>
    where D: Device, P: OutputPin {}

impl<D, P> NotificationHandler<Lifecycle> for SimpleLED<D, P>
    where D: Device, P: OutputPin
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D, P> NotificationHandler<On> for SimpleLED<D, P>
    where
        Self: Switchable<D>, D: Device, P: OutputPin
{
    fn on_notification(&'static mut self, message: On) -> Completion {
        self.turn_on();
        Completion::immediate()
    }
}

impl<D, P> NotificationHandler<Off> for SimpleLED<D, P>
    where
        Self: Switchable<D>,
        D: Device,
        P: OutputPin
{
    fn on_notification(&'static mut self, message: Off) -> Completion {
        Completion::defer(async move {
            self.turn_off();
        })
    }
}

impl<D, S> Address<D, S>
    where
        S: NotificationHandler<Off>, D: Device + 'static,
        S: Actor<D> + 'static,
        S: Switchable<D>,
        S: NotificationHandler<On>,
{
    pub fn turn_on(&self) {
        self.notify(On);
    }

    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
