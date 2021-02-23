use crate::hal::gpio::InterruptPeripheral;
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
    bus: Option<Address<EventBus<D>>>,
}

impl<D, PIN> Actor for Button<D, PIN>
where
    D: Device,
    PIN: InputPin + InterruptPeripheral,
{
    type Configuration = Address<EventBus<D>>;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.bus.replace(config);
    }
}

impl<D, PIN> Button<D, PIN>
where
    D: Device,
    PIN: InputPin + InterruptPeripheral,
{
    pub fn new(pin: PIN, active: Active) -> Self {
        Self {
            pin,
            active,
            bus: None,
        }
    }
}

impl<D, PIN> Interrupt for Button<D, PIN>
where
    D: Device + EventHandler<ButtonEvent> + 'static,
    PIN: InputPin + InterruptPeripheral,
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            match self.active {
                Active::High => {
                    if self.pin.is_high().ok().unwrap() {
                        self.bus.unwrap().publish(ButtonEvent::Pressed);
                    } else {
                        self.bus.unwrap().publish(ButtonEvent::Released);
                    }
                }
                Active::Low => {
                    if self.pin.is_low().ok().unwrap() {
                        self.bus.unwrap().publish(ButtonEvent::Pressed);
                    } else {
                        self.bus.unwrap().publish(ButtonEvent::Released);
                    }
                }
            }
            self.pin.clear_interrupt();
        }
    }
}
