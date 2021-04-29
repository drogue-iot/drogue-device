use core::future::Future;
use core::pin::Pin;
use crate::kernel::actor::{Actor, Address};
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
    type Message<'m> where 'a: 'm = ();
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
                    actor.notify(self.message).await;
                }
            }
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        _: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {}
    }
}