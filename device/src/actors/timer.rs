use crate::kernel::actor::{Actor, Address, Inbox};
use core::future::Future;
use core::pin::Pin;
use embassy::time;

pub struct Timer<'a, A: Actor + 'static> {
    _marker: core::marker::PhantomData<&'a A>,
}

pub enum TimerMessage<'m, A: Actor + 'static> {
    Delay(time::Duration),
    Schedule(time::Duration, Address<'m, A>, Option<A::Message<'m>>),
}

impl<'m, A: Actor + 'm> TimerMessage<'m, A> {
    pub fn delay(duration: time::Duration) -> Self {
        TimerMessage::Delay(duration)
    }

    pub fn schedule(
        duration: time::Duration,
        destination: Address<'m, A>,
        message: A::Message<'m>,
    ) -> Self {
        TimerMessage::Schedule(duration, destination, Some(message))
    }
}

impl<'a, A: Actor + 'a> Timer<'a, A> {
    pub fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a, A: Actor + 'a> Actor for Timer<'a, A> {
    #[rustfmt::skip]
    type Message<'m> where 'a: 'm = TimerMessage<'m, A>;
    #[rustfmt::skip]
    type OnStartFuture<'m, M> where 'a: 'm, M: 'm = impl Future<Output = ()> + 'm;

    fn on_start<'m, M>(self: Pin<&'m mut Self>, inbox: &'m mut M) -> Self::OnStartFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                if let Some((message, responder)) = inbox.next().await {
                    responder.respond(match message {
                        TimerMessage::Delay(dur) => {
                            time::Timer::after(dur).await;
                        }
                        TimerMessage::Schedule(dur, address, mut message) => {
                            time::Timer::after(dur).await;
                            let _ = address.notify(message.take().unwrap());
                        }
                    });
                }
            }
        }
    }
}
