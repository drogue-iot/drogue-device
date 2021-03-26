use crate::api::timer::Timer;
use crate::api::switchable::Switchable;
use crate::domain::time::duration::Milliseconds;
use crate::prelude::*;

pub struct Blinker<S, T>
where
    S: Timer + 'static,
    T: Timer + 'static,
{
    led: Option<Address<S>>,
    timer: Option<Address<T>>,
    delay: Milliseconds,
    address: Option<Address<Self>>,
}

impl<S, T> Blinker<S, T>
where
    S: Switchable,
    T: Timer,
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

impl<S, T> Actor for Blinker<S, T>
where
    S: Switchable,
    T: Timer,
{
    type Configuration = (Address<S>, Address<T>);
    type Request = BlinkRequest;
    type Response = ();

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.address.replace(address);
        self.led.replace(config.0);
        self.timer.replace(config.1);
    }

    fn on_start(self) -> Completion<Self> {
        self.timer
            .unwrap()
            .schedule(self.delay, BlinkRequest::On, self.address.unwrap());
        Completion::immediate(self)
    }

    fn on_request(self, message: Self::Request) -> Response<Self> {
        match message {
            BlinkRequest::On => {
                self.led.unwrap().turn_on();
                self.timer
                    .unwrap()
                    .schedule(self.delay, BlinkRequest::Off, self.address.unwrap());
            }
            BlinkRequest::Off => {
                self.led.unwrap().turn_off();
                self.timer
                    .unwrap()
                    .schedule(self.delay, BlinkRequest::On, self.address.unwrap());
            }
            BlinkRequest::AdjustDelay(ms) => {
                self.delay = message.0;
            }
        }
        Response::immediate(self, ())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BlinkRequest {
    On,
    Off,
    AdjustDelay(Milliseconds),
}

impl<S, T> Address<Blinker<S, T>>
where
    Self: 'static,
    S: Switchable,
    T: Timer,
{
    pub fn adjust_delay(&self, delay: Milliseconds) {
        self.notify(BlinkRequest::AdjustDelay(delay))
    }
}
