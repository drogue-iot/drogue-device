use crate::api::switchable::{Switch, Switchable};
use crate::hal::gpio::ActiveOutput;
use crate::hal::Active;
use crate::prelude::*;
use core::marker::PhantomData;
use embedded_hal::digital::v2::OutputPin;

pub struct SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    pin: P,
    _active: PhantomData<A>,
}

impl<P, A> Switchable for SimpleLED<P, A>
where
    P: OutputPin + 'static,
    A: ActiveOutput + 'static,
{
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

    fn turn_on(&mut self) {
        A::set_active(&mut self.pin).ok();
    }

    fn turn_off(&mut self) {
        A::set_inactive(&mut self.pin).ok();
    }
}

impl<P, A> Actor for SimpleLED<P, A>
where
    P: OutputPin,
    A: ActiveOutput,
{
    type Configuration = ();
    type Request = Switch;
    type Response = ();

    fn on_request(mut self, message: Self::Request) -> Response<Self> {
        match message {
            Switch::On => self.turn_on(),
            Switch::Off => self.turn_off(),
        }
        Response::immediate(self, ())
    }
}
