use cortex_m::interrupt::Nr;
use stm32l4xx_hal::gpio::ExtiPin;
use stm32l4xx_hal::hal::digital::v2::InputPin;
use drogue_device::prelude::*;
use crate::device::ButtonToLed;

#[derive(Copy, Clone)]
pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct Button<PIN> {
    pin: PIN,
}

impl<D: Device, PIN: InputPin + ExtiPin> Actor<D> for Button<PIN> {
}

impl<PIN: InputPin + ExtiPin> NotificationHandler<Lifecycle> for Button<PIN> {
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<PIN: InputPin + ExtiPin> Button<PIN> {
    pub fn new(pin: PIN) -> Self {
        Self {
            pin,
        }
    }
}


impl<D: Device, PIN: InputPin + ExtiPin> Interrupt<D> for Button<PIN> {
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            log::info!("button pressed");
            self.pin.clear_interrupt_pending_bit();
        }
    }
}
