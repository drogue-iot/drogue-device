use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;

pub struct On;
pub struct Off;

pub struct SimpleLED<PIN: OutputPin> {
    pin: PIN,
}

impl<PIN: OutputPin> SimpleLED<PIN> {
    pub fn new(pin: PIN) -> Self {
        Self { pin }
    }

    pub fn turn_on(&mut self) {
        self.pin.set_high().ok().unwrap();
    }

    pub fn turn_off(&mut self) {
        self.pin.set_low().ok().unwrap();
    }
}

impl<PIN: OutputPin> Actor for SimpleLED<PIN> {

}

impl<PIN: OutputPin> NotificationHandler<On> for SimpleLED<PIN> {
    fn on_notification(&'static mut self, message: On) -> Completion {
        self.turn_on();
        Completion::immediate()
    }
}

impl<PIN: OutputPin> NotificationHandler<Off> for SimpleLED<PIN> {
    fn on_notification(&'static mut self, message: Off) -> Completion {
        Completion::defer( async move {
            self.turn_off();
        })
    }
}

impl<PIN: OutputPin + 'static> Address<SimpleLED<PIN>> {
    pub fn turn_on(&self) {
        self.notify(On);
    }

    pub fn turn_off(&self) {
        self.notify(Off);
    }
}
