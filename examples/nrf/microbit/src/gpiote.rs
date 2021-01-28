use drogue_device::prelude::*;
use embedded_hal::digital::v2::InputPin;

use nrf52833_hal as hal;

use hal::gpiote::GpioteInputPin;

pub struct Gpiote<D: Device + EventConsumer<GpioteEvent>> {
    gpiote: hal::gpiote::Gpiote,
    bus: Option<EventBus<D>>,
}

pub struct GpioteChannel<
    D: Device + EventConsumer<PinEvent>,
    P: InputPin + GpioteInputPin + 'static,
> {
    bus: Option<EventBus<D>>,
    channel: Channel,
    pin: P,
}

impl<D: Device + EventConsumer<PinEvent>, P: InputPin + GpioteInputPin + Sized> Actor<D>
    for GpioteChannel<D, P>
{
    fn mount(&mut self, _: Address<D, Self>, bus: EventBus<D>) {
        self.bus.replace(bus);
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub struct PinEvent(pub Channel, pub PinState);

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum PinState {
    High,
    Low,
}

#[allow(dead_code)]
pub enum Edge {
    Rising,
    Falling,
    Both,
}

impl<D: Device + EventConsumer<GpioteEvent>> Gpiote<D> {
    pub fn new(gpiote: hal::pac::GPIOTE) -> Self {
        let gpiote = hal::gpiote::Gpiote::new(gpiote);
        Self { gpiote, bus: None }
    }

    pub fn configure_channel<P: InputPin + GpioteInputPin>(
        &self,
        channel: Channel,
        pin: P,
        edge: Edge,
    ) -> GpioteChannel<D, P>
    where
        D: EventConsumer<PinEvent>,
    {
        let ch = match channel {
            Channel::Channel0 => self.gpiote.channel0(),
            Channel::Channel1 => self.gpiote.channel1(),
            Channel::Channel2 => self.gpiote.channel2(),
            Channel::Channel3 => self.gpiote.channel3(),
        };

        let che = ch.input_pin(&pin);

        match edge {
            Edge::Rising => che.lo_to_hi(),
            Edge::Falling => che.hi_to_lo(),
            Edge::Both => che.toggle(),
        };

        che.enable_interrupt();
        GpioteChannel::new(channel, pin)
    }
}

impl<D: Device + EventConsumer<PinEvent>, P: InputPin + GpioteInputPin> GpioteChannel<D, P> {
    pub fn new(channel: Channel, pin: P) -> GpioteChannel<D, P> {
        GpioteChannel {
            channel,
            pin,
            bus: None,
        }
    }
}

impl<D: Device + EventConsumer<PinEvent>, P: InputPin + GpioteInputPin>
    NotificationHandler<GpioteEvent> for GpioteChannel<D, P>
{
    fn on_notification(&'static mut self, event: GpioteEvent) -> Completion {
        match event {
            GpioteEvent(c) if c == self.channel => {
                log::info!("Channel {:?} notified!", self.channel);
                if let Some(bus) = &self.bus {
                    if self.pin.is_high().ok().unwrap() {
                        bus.publish(PinEvent(c, PinState::High));
                    } else {
                        bus.publish(PinEvent(c, PinState::Low));
                    }
                }
            }
            _ => {}
        }
        Completion::immediate()
    }
}

impl<D: Device + EventConsumer<GpioteEvent> + 'static> Interrupt<D> for Gpiote<D> {
    fn on_interrupt(&mut self) {
        log::info!("GPIOTE INTERRUPT");
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

impl<D: Device + EventConsumer<GpioteEvent>> Actor<D> for Gpiote<D> {
    fn mount(&mut self, _: Address<D, Self>, bus: EventBus<D>) {
        self.bus.replace(bus);
    }
}

impl<D: Device + EventConsumer<GpioteEvent>> NotificationHandler<Lifecycle> for Gpiote<D> {
    fn on_notification(&'static mut self, _: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D: Device + EventConsumer<PinEvent>, P: InputPin + GpioteInputPin + 'static>
    NotificationHandler<Lifecycle> for GpioteChannel<D, P>
{
    fn on_notification(&'static mut self, _: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

/*
impl<D: Device + EventConsumer<PinEvent>, P: InputPin + GpioteInputPin + 'static>
    EventProducer<D, PinEvent> for GpioteChannel<D, P>
{
}

impl<D: Device + EventConsumer<GpioteEvent>> EventProducer<D, GpioteEvent> for Gpiote<D> {}
*/

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum Channel {
    Channel0,
    Channel1,
    Channel2,
    Channel3,
}

#[derive(Debug, Copy, Clone)]
pub struct GpioteEvent(pub Channel);
