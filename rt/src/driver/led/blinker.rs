use crate::api::scheduler::Scheduler;
use crate::api::switchable::Switchable;
use crate::domain::time::duration::Milliseconds;
use crate::prelude::*;

pub struct Blinker<S, T>
where
    S: Switchable + 'static,
    T: Scheduler + 'static,
{
    led: Option<Address<S>>,
    timer: Option<Address<T>>,
    delay: Milliseconds,
    address: Option<Address<Self>>,
}

impl<S, T> Blinker<S, T>
where
    S: Switchable,
    T: Scheduler,
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
    T: Scheduler,
{
    type Configuration = (Address<S>, Address<T>);

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
            .schedule(self.delay, State::On, self.address.unwrap());
        Completion::immediate(self)
    }
}

#[derive(Copy, Clone, Debug)]
enum State {
    On,
    Off,
}

impl<S, T> RequestHandler<State> for Blinker<S, T>
where
    S: Switchable,
    T: Scheduler,
{
    type Response = ();
    fn on_request(self, message: State) -> Response<Self, Self::Response> {
        match message {
            State::On => {
                self.led.unwrap().turn_on();
                self.timer
                    .unwrap()
                    .schedule(self.delay, State::Off, self.address.unwrap());
            }
            State::Off => {
                self.led.unwrap().turn_off();
                self.timer
                    .unwrap()
                    .schedule(self.delay, State::On, self.address.unwrap());
            }
        }
        Response::immediate(self, ())
    }
}

pub struct AdjustDelay(Milliseconds);

impl<S, T> RequestHandler<AdjustDelay> for Blinker<S, T>
where
    S: Switchable,
    T: Scheduler,
{
    type Response = ();
    fn on_request(mut self, message: AdjustDelay) -> Response<Self, Self::Response> {
        self.delay = message.0;
        Response::immediate(self, ())
    }
}

impl<S, T> Address<Blinker<S, T>>
where
    Self: 'static,
    S: Switchable,
    T: Scheduler,
{
    pub fn adjust_delay(&self, delay: Milliseconds) {
        self.notify(AdjustDelay(delay))
    }
}
