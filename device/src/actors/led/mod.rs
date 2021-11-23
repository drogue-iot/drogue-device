#[cfg(feature = "time")]
pub mod matrix;

use crate::{
    actors::button::{ButtonEvent, ButtonEventHandler},
    kernel::{actor::Actor, actor::Address, actor::Inbox},
};
use core::future::Future;
use core::marker::PhantomData;
use embedded_hal::digital::v2::{OutputPin, PinState};

pub trait Active<P>
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error>;
}

pub struct ActiveHigh;
pub struct ActiveLow;

impl<P> Active<P> for ActiveHigh
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error> {
        pin.set_state(if active {
            PinState::High
        } else {
            PinState::Low
        })
    }
}

impl<P> Active<P> for ActiveLow
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error> {
        pin.set_state(if active {
            PinState::Low
        } else {
            PinState::High
        })
    }
}

#[derive(Clone, Copy)]
pub enum LedMessage {
    On,
    Off,
    Toggle,
    State(bool),
}

pub struct Led<P, ACTIVE = ActiveHigh>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    pin: P,
    state: bool,
    _active: PhantomData<ACTIVE>,
}

impl<P, ACTIVE> Led<P, ACTIVE>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            state: false,
            _active: PhantomData,
        }
    }
}

impl<P, ACTIVE> ButtonEventHandler for Address<'static, Led<P, ACTIVE>>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    fn handle(&mut self, event: ButtonEvent) {
        let _ = match event {
            ButtonEvent::Pressed => self.notify(LedMessage::On),
            ButtonEvent::Released => self.notify(LedMessage::Off),
        };
    }
}

impl<P, ACTIVE> Actor for Led<P, ACTIVE>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    type Message<'m>
    where
        Self: 'm,
    = LedMessage;

    type OnMountFuture<'m, M>
    where
        Self: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let new_state = match *m.message() {
                        LedMessage::On => true,
                        LedMessage::Off => false,
                        LedMessage::State(state) => state,
                        LedMessage::Toggle => !self.state,
                    };
                    if self.state != new_state {
                        match ACTIVE::set(&mut self.pin, self.state) {
                            Ok(_) => {
                                self.state = new_state;
                            }
                            Err(_) => {
                                // ignore, didn't work, don't update state.
                            }
                        }
                    }
                }
            }
        }
    }
}
