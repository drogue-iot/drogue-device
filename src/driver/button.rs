use embedded_hal::digital::v2::InputPin;
use crate::prelude::*;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::hal::Active;

#[derive(Copy, Clone)]
pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct Button<D, PIN>
    where
        D: Device + EventConsumer<ButtonEvent>,
{
    pin: PIN,
    active: Active,
    bus: Option<EventBus<D>>,
}

impl<D, PIN> Actor<D>
for Button<D, PIN>
    where
        D: Device + EventConsumer<ButtonEvent>,
        PIN: InputPin + ExtiPin
{
    fn mount(&mut self, address: Address<D, Self>, bus: EventBus<D>) where
        Self: Sized,
    {
        self.bus.replace(bus);
    }
}

impl<D, PIN> NotificationHandler<Lifecycle>
for Button<D, PIN>
    where
        D: Device + EventConsumer<ButtonEvent>,
        PIN: InputPin + ExtiPin
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D, PIN> Button<D, PIN>
    where
        D: Device + EventConsumer<ButtonEvent>,
        PIN: InputPin + ExtiPin
{
    pub fn new(pin: PIN, active: Active) -> Self {
        Self {
            pin,
            active,
            bus: None,
        }
    }
}


impl<D, PIN> Interrupt<D> for Button<D, PIN>
    where
        D: Device + EventConsumer<ButtonEvent> + 'static,
        PIN: InputPin + ExtiPin
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            match self.active {
                Active::High => {
                    if self.pin.is_high().ok().unwrap() {
                        self.bus.as_ref().unwrap().publish( ButtonEvent::Pressed );
                    } else {
                        self.bus.as_ref().unwrap().publish( ButtonEvent::Released );
                    }
                }
                Active::Low => {
                    if self.pin.is_low().ok().unwrap() {
                        self.bus.as_ref().unwrap().publish( ButtonEvent::Pressed );
                    } else {
                        self.bus.as_ref().unwrap().publish( ButtonEvent::Released );
                    }
                }
            }
            self.pin.clear_interrupt_pending_bit();
        }
    }
}
