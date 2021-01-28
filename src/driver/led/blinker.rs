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
        T: HalTimer {}

impl<D, P, T> NotificationHandler<Lifecycle>
for Blinker<D, P, T>
    where
        D: Device,
        P: OutputPin,
        T: HalTimer,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        if let Lifecycle::Start = message {
            Completion::defer(async move {
                loop {
                    //log::info!("LED {:?}", self.delay);
                    self.led.as_ref().unwrap().turn_on();
                    self.timer.as_ref().unwrap().delay(self.delay).await;
                    self.led.as_ref().unwrap().turn_off();
                    self.timer.as_ref().unwrap().delay(self.delay).await;
                }
            })
        } else {
            Completion::immediate()
        }
    }
}
