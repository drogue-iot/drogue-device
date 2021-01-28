use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::driver::led::SimpleLED;
use crate::driver::timer::{HardwareTimer, Timer};
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;

pub struct Blinker<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> {
    led: Option<Address<D, SimpleLED<D, P>>>,
    timer: Option<Address<D, Timer<D, TIM, T>>>,
    delay: Milliseconds,
}

impl<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> Blinker<D, P, TIM, T> {
    pub fn new<DUR: Into<Milliseconds>>(delay: DUR) -> Self {
        Self {
            led: None,
            timer: None,
            delay: delay.into(),
        }
    }
}

impl<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> Bind<D, SimpleLED<D, P>>
    for Blinker<D, P, TIM, T>
{
    fn on_bind(&'static mut self, address: Address<D, SimpleLED<D, P>>) {
        self.led.replace(address);
    }
}

impl<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> Bind<D, Timer<D, TIM, T>>
    for Blinker<D, P, TIM, T>
{
    fn on_bind(&'static mut self, address: Address<D, Timer<D, TIM, T>>) {
        self.timer.replace(address);
    }
}

impl<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> Actor<D> for Blinker<D, P, TIM, T> {}

impl<D: Device, P: OutputPin, TIM, T: HardwareTimer<TIM>> NotificationHandler<Lifecycle>
    for Blinker<D, P, TIM, T>
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
