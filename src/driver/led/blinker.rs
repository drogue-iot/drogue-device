use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::driver::led::SimpleLED;
use crate::hal::timer::Timer as HalTimer;
use crate::driver::timer::Timer;
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;


pub struct Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    led: Option<Address<SimpleLED<P>>>,
    timer: Option<Address<Timer<T>>>,
    delay: Milliseconds,
    address: Option<Address<Self>>,
}

impl<P, T> Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    pub fn new<DUR: Into<Milliseconds>>(delay: DUR) -> Self {
        Self {
            led: None,
            timer: None,
            delay: delay.into(),
            address: None,
        }
    }
}

impl<P, T> Bind<SimpleLED<P>>
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    fn on_bind(&'static mut self, address: Address<SimpleLED<P>>) {
        self.led.replace(address);
    }
}

impl<P, T> Bind<Timer<T>>
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer {
    fn on_bind(&'static mut self, address: Address<Timer<T>>) {
        self.timer.replace(address);
    }
}

impl<P, T> Actor
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer {
    fn mount(&mut self, address: Address<Self>)
        where
            Self: Sized, {
        self.address.replace(address);
    }
}

impl<P, T> NotificationHandler<Lifecycle>
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        self.timer.as_ref().unwrap().schedule(self.delay, State::On, self.address.as_ref().unwrap().clone());
        Completion::immediate()
    }
}

#[derive(Copy, Clone, Debug)]
enum State {
    On,
    Off,
}

impl<P, T> NotificationHandler<State>
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    fn on_notification(&'static mut self, message: State) -> Completion {
        match message {
            State::On => {
                self.led.as_ref().unwrap().turn_on();
                self.timer.as_ref().unwrap().schedule(self.delay, State::Off, self.address.as_ref().unwrap().clone());
            }
            State::Off => {
                self.led.as_ref().unwrap().turn_off();
                self.timer.as_ref().unwrap().schedule(self.delay, State::On, self.address.as_ref().unwrap().clone());
            }
        }
        Completion::immediate()
    }
}

pub struct AdjustDelay(Milliseconds);

impl<P, T> NotificationHandler<AdjustDelay>
for Blinker<P, T>
    where
        P: OutputPin,
        T: HalTimer,
{
    fn on_notification(&'static mut self, message: AdjustDelay) -> Completion {
        self.delay = message.0;
        Completion::immediate()
    }
}


impl<P, T> Address<Blinker<P, T>>
    where
        Self: 'static,
        P: OutputPin,
        T: HalTimer,
{
    pub fn adjust_delay(&self, delay: Milliseconds) {
        self.notify(AdjustDelay(delay))
    }
}
