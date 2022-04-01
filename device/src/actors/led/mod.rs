use crate::actors::button::ButtonEvent;
use crate::traits;
use crate::{Actor, Address, Inbox};
use core::convert::TryFrom;
use core::future::Future;

#[derive(Clone, Copy)]
pub enum LedMessage {
    On,
    Off,
    Toggle,
    State(bool),
}

pub struct Led<P>
where
    P: traits::led::Led,
{
    led: P,
    state: bool,
}

impl<P> Led<P>
where
    P: traits::led::Led,
{
    pub fn new(led: P) -> Self {
        Self { led, state: false }
    }
}

impl TryFrom<ButtonEvent> for LedMessage {
    type Error = ();
    fn try_from(event: ButtonEvent) -> Result<LedMessage, Self::Error> {
        match event {
            ButtonEvent::Pressed => Ok(LedMessage::On),
            ButtonEvent::Released => Ok(LedMessage::Off),
        }
    }
}

impl<P> Actor for Led<P>
where
    P: traits::led::Led,
{
    type Message<'m> = LedMessage;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<LedMessage> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<LedMessage>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<LedMessage> + 'm,
    {
        async move {
            loop {
                let new_state = match inbox.next().await {
                    LedMessage::On => true,
                    LedMessage::Off => false,
                    LedMessage::State(state) => state,
                    LedMessage::Toggle => !self.state,
                };
                if self.state != new_state {
                    match match new_state {
                        true => self.led.on(),
                        false => self.led.off(),
                    } {
                        Ok(_) => {
                            self.state = new_state;
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }
}
