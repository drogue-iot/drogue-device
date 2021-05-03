use crate::kernel::actor::{Actor, Address};
use core::future::Future;
use core::pin::Pin;
use embassy::time::{Duration, Timer};

pub struct Ticker<'a, A: Actor + 'a>
where
    A::Message<'a>: Copy,
{
    interval: Duration,
    message: A::Message<'a>,
    actor: Option<Address<'a, A>>,
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
        }
    }
}

impl<'a, A: Actor + 'a> Actor for Ticker<'a, A>
where
    A::Message<'a>: Copy,
{
    type Configuration = Address<'a, A>;
    #[rustfmt::skip]
    type OnStartFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.actor.replace(config);
    }

    fn on_start(self: Pin<&mut Self>) -> Self::OnStartFuture<'_> {
        async move {
            loop {
                Timer::after(self.interval).await;
                if let Some(actor) = self.actor {
                    // We continue even if we get an error, trying again
                    // next tick.
                    let _ = actor.notify(self.message);
                }
            }
        }
    }

    fn on_message<'m>(self: Pin<&'m mut Self>, _: Self::Message<'m>) -> Self::OnMessageFuture<'m> {
        async move {}
    }
}
