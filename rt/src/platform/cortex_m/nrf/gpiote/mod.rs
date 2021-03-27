use crate::{driver::button::*, prelude::*};
use embedded_hal::digital::v2::InputPin;

#[cfg(feature = "chip+nrf52833")]
use nrf52833_hal as hal;

#[cfg(feature = "chip+nrf51")]
use nrf51_hal as hal;

use hal::gpiote::GpioteInputPin;

pub struct Gpiote<D>
where
    D: Device + EventHandler<GpioteEvent> + 'static,
{
    gpiote: hal::gpiote::Gpiote,
    bus: Option<EventBus<D>>,
}

impl<D: Device + EventHandler<GpioteEvent>> Actor for Gpiote<D> {
    type Configuration = EventBus<D>;
    type Request = ();
    type Response = ();
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.bus.replace(config);
    }

    fn on_request(self, _: Self::Request) -> Response<Self> {
        Response::immediate(self, ())
    }


}

impl<D: Device + EventHandler<GpioteEvent>> Gpiote<D> {
    pub fn new(gpiote: hal::gpiote::Gpiote) -> Self {
        Self { gpiote, bus: None }
    }
}

impl<D: Device + EventHandler<GpioteEvent> + 'static> Interrupt for Gpiote<D> {
    fn on_interrupt(&mut self) {
        if let Some(bus) = &self.bus {
            if self.gpiote.channel0().is_event_triggered() {
                bus.publish(GpioteEvent(Channel::Channel0));
            }

            if self.gpiote.channel1().is_event_triggered() {
                bus.publish(GpioteEvent(Channel::Channel1));
            }

            if self.gpiote.channel2().is_event_triggered() {
                bus.publish(GpioteEvent(Channel::Channel2));
            }

            if self.gpiote.channel3().is_event_triggered() {
                bus.publish(GpioteEvent(Channel::Channel3));
            }
        }
        self.gpiote.reset_events();
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum Channel {
    Channel0,
    Channel1,
    Channel2,
    Channel3,
}

#[derive(Debug, Copy, Clone)]
pub struct GpioteEvent(pub Channel);
