use crate::hal::gpio::InterruptPin;
use crate::hal::Active;
use crate::prelude::*;
use embedded_hal::digital::v2::InputPin;

#[derive(Copy, Clone)]
pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct Button<D: Device + 'static, PIN> {
    pin: PIN,
    active: Active,
    bus: Option<EventBus<D>>,
}

pub struct PinInterrupt;

impl<D, PIN> Actor for Button<D, PIN>
where
    D: Device + EventHandler<ButtonEvent> + 'static,
    PIN: InputPin,
{
    type Configuration = EventBus<D>;
    type Request = PinInterrupt;
    type Response = ();

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.bus.replace(config);
    }

    fn on_request(self, interrupt: Self::Request) -> Response<Self> {
        self.check_pin();
        Response::immediate(self, ())
    }
}

impl<D, PIN> Button<D, PIN>
where
    D: Device + EventHandler<ButtonEvent> + 'static,
    PIN: InputPin,
{
    pub fn new(pin: PIN, active: Active) -> Self {
        Self {
            pin,
            active,
            bus: None,
        }
    }

    pub fn notify_high(&self) {
        match self.active {
            Active::High => self.bus.unwrap().publish(ButtonEvent::Pressed),
            _ => self.bus.unwrap().publish(ButtonEvent::Released),
        }
    }

    pub fn notify_low(&self) {
        match self.active {
            Active::Low => self.bus.unwrap().publish(ButtonEvent::Pressed),
            _ => self.bus.unwrap().publish(ButtonEvent::Released),
        }
    }

    pub fn check_pin(&self) {
        if self.pin.is_high().ok().unwrap() {
            self.notify_high();
        } else {
            self.notify_low();
        }
    }
}

impl<D, PIN> Interrupt for Button<D, PIN>
where
    D: Device + EventHandler<ButtonEvent> + 'static,
    PIN: InputPin + InterruptPin,
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            self.check_pin();

            self.pin.clear_interrupt();
        }
    }
}
