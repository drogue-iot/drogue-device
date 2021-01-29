use embedded_hal::digital::v2::InputPin;
use crate::prelude::*;
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::hal::Active;
use crate::bind::Bind;
use crate::handler::EventHandler;

#[derive(Copy, Clone)]
pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct Button<D: Device, PIN>
{
    pin: PIN,
    active: Active,
    bus: Option<Address<EventBus<D>>>,
}

impl<D, PIN> Actor
for Button<D, PIN>
    where
        D: Device,
        PIN: InputPin + ExtiPin
{
    fn mount(&mut self, address: Address<Self>) where
        Self: Sized,
    {
        //self.bus.replace(bus);
    }
}

impl<D, PIN> Bind<EventBus<D>>
for Button<D, PIN>
    where
        D: Device
{
    fn on_bind(&'static mut self, address: Address<EventBus<D>>) {
        self.bus.replace(address);
    }
}

impl<D, PIN> Button<D, PIN>
    where
        D: Device,
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


impl<D, PIN> Interrupt for Button<D, PIN>
    where
        D: Device + EventHandler<ButtonEvent> + 'static,
        PIN: InputPin + ExtiPin
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            match self.active {
                Active::High => {
                    if self.pin.is_high().ok().unwrap() {
                        self.bus.as_ref().unwrap().publish(ButtonEvent::Pressed);
                    } else {
                        self.bus.as_ref().unwrap().publish(ButtonEvent::Released);
                    }
                }
                Active::Low => {
                    if self.pin.is_low().ok().unwrap() {
                        self.bus.as_ref().unwrap().publish(ButtonEvent::Pressed);
                    } else {
                        self.bus.as_ref().unwrap().publish(ButtonEvent::Released);
                    }
                }
            }
            self.pin.clear_interrupt_pending_bit();
        }
    }
}
