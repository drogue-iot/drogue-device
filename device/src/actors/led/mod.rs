#[cfg(feature = "time")]
pub mod matrix;

use crate::{
    actors::button::{ButtonEvent, FromButtonEvent},
    kernel::{actor::Actor, actor::Address, actor::Inbox},
};
use core::future::Future;
use embedded_hal::digital::v2::OutputPin;

#[derive(Clone, Copy)]
pub enum LedMessage {
    On,
    Off,
    Toggle,
    State(bool),
}

impl<P> FromButtonEvent<LedMessage> for Led<P>
where
    P: OutputPin,
{
    fn from(event: ButtonEvent) -> Option<LedMessage> {
        Some(match event {
            ButtonEvent::Pressed => LedMessage::On,
            ButtonEvent::Released => LedMessage::Off,
        })
    }
}

pub struct Led<P>
where
    P: OutputPin,
{
    pin: P,

    state: bool,
}

impl<P> Led<P>
where
    P: OutputPin,
{
    pub fn new(pin: P) -> Self {
        Self { pin, state: false }
    }
}

impl<P> Unpin for Led<P> where P: OutputPin {}

impl<P> Actor for Led<P>
where
    P: OutputPin,
{
    #[rustfmt::skip]
    type Message<'m> where Self: 'm = LedMessage;
    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm= impl Future<Output = ()> + 'm;

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
                        self.state = new_state;
                        match self.state {
                            true => self.pin.set_high().ok(),
                            false => self.pin.set_low().ok(),
                        };
                    }
                }
            }
        }
    }
}
