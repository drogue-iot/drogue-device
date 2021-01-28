use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::driver::led::SimpleLED;
use crate::hal::timer::Timer as HalTimer;
use crate::driver::timer::Timer;
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;


pub struct Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer,
{
    led: Option<Address<D, SimpleLED<D, P>>>,
    timer: Option<Address<D, Timer<D, T>>>,
    delay: Milliseconds,
    address: Option<Address<D, Self>>,
}

impl<D, P, T> Blinker<D, P, T>
    where
        D: Device,
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

impl<D, P, T> Bind<D, SimpleLED<D, P>>
for Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer,
{
    fn on_bind(&'static mut self, address: Address<D, SimpleLED<D, P>>) {
        self.led.replace(address);
    }
}

impl<D, P, T> Bind<D, Timer<D, T>>
for Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer {
    fn on_bind(&'static mut self, address: Address<D, Timer<D, T>>) {
        self.timer.replace(address);
    }
}

impl<D, P, T> Actor<D>
for Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer {
    fn mount(&mut self, address: Address<D, Self>, bus: EventBus<D>)
        where
            Self: Sized, {
        self.address.replace(address);
    }
}

impl<D, P, T> NotificationHandler<Lifecycle>
for Blinker<D, P, T>
    where
        D: Device,
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

impl<D, P, T> NotificationHandler<State>
for Blinker<D, P, T>
    where
        D: Device,
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

impl<D, P, T> NotificationHandler<AdjustDelay>
for Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer,
{
    fn on_notification(&'static mut self, message: AdjustDelay) -> Completion {
        self.delay = message.0;
        Completion::immediate()
    }
}


impl<D, P, T> Address<D, Blinker<D, P, T>>
    where
        Self: 'static,
        D: Device,
        P: OutputPin,
        T: HalTimer,
{
    pub fn adjust_delay(&self, delay: Milliseconds) {
        self.notify(AdjustDelay(delay))
    }
}
