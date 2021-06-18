use crate::kernel::actor::{Actor, Address};
use core::future::Future;
use core::pin::Pin;
use embassy::time::{Duration, Timer};
use heapless::consts;

pub struct Ticker<'a, A: Actor + 'static>
where
    A::Message<'a>: Copy,
    Self: 'static,
{
    interval: Duration,
    message: A::Message<'a>,
    actor: Option<Address<'a, A>>,
    me: Option<Address<'a, Self>>,
    running: bool,
}

impl<'a, A: Actor + 'a> Ticker<'a, A>
where
    A::Message<'a>: Copy,
{
    pub fn new(interval: Duration, message: A::Message<'a>) -> Self {
        Self {
            interval,
            message,
            actor: None,
            me: None,
            running: false,
        }
    }
}

pub enum TickerCommand {
    Tick,
    Start,
    Stop,
}

impl<'a, A: Actor + 'a> Actor for Ticker<'a, A>
where
    A::Message<'a>: Copy,
{
    type MessageQueueSize<'m> = consts::U2;
    type Configuration = Address<'a, A>;
    #[rustfmt::skip]
    type Message<'m> where 'a: 'm = TickerCommand;
    #[rustfmt::skip]
    type OnStartFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, me: Address<'a, Self>, config: Self::Configuration) {
        self.me.replace(me);
        self.actor.replace(config);
    }

    fn on_start(self: Pin<&mut Self>) -> Self::OnStartFuture<'_> {
        async move {
            self.me.unwrap().notify(TickerCommand::Start).unwrap();
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let this = unsafe { self.get_unchecked_mut() };
            match message {
                TickerCommand::Tick => {
                    if this.running {
                        // Wait the configured interval before sending the message
                        Timer::after(this.interval).await;
                        if let Some(actor) = this.actor {
                            // We continue even if we get an error, trying again
                            // next tick.
                            let _ = actor.notify(this.message);
                        }
                        this.me.unwrap().notify(TickerCommand::Tick).unwrap();
                    }
                }
                TickerCommand::Start => {
                    this.running = true;
                    this.me.unwrap().notify(TickerCommand::Tick).unwrap();
                }
                TickerCommand::Stop => {
                    this.running = false;
                }
            }
        }
    }
}
