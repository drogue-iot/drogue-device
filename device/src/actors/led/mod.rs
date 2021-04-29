pub mod matrix;

use crate::kernel::{actor::Actor, channel::consts, util::ImmediateFuture};
use core::pin::Pin;
use embedded_hal::digital::v2::OutputPin;

pub enum LedMessage {
    On,
    Off,
    Toggle,
    State(bool),
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
    type MaxMessageQueueSize<'m> where Self: 'm = consts::U1;
    type Configuration = ();
    #[rustfmt::skip]
    type Message<'m> where Self: 'm = LedMessage;
    #[rustfmt::skip]
    type OnStartFuture<'m> where Self: 'm= ImmediateFuture;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where Self: 'm = ImmediateFuture;

    fn on_start(self: Pin<&mut Self>) -> Self::OnStartFuture<'_> {
        ImmediateFuture::new()
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        msg: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        let new_state = match msg {
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

        ImmediateFuture::new()
    }
}
