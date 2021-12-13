#[cfg(feature = "time")]
pub mod matrix;

use crate::traits;
use crate::{
    actors::button::{ButtonEvent, ButtonEventHandler},
    kernel::{actor::Actor, actor::Address, actor::Inbox},
};
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

impl<P> ButtonEventHandler for Address<Led<P>>
where
    P: traits::led::Led,
{
    fn handle(&mut self, event: ButtonEvent) {
        let _ = match event {
            ButtonEvent::Pressed => self.notify(LedMessage::On),
            ButtonEvent::Released => self.notify(LedMessage::Off),
        };
    }
}

impl<P> Actor for Led<P>
where
    P: traits::led::Led,
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
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
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
}
