use core::future::Future;
use core::pin::Pin;
use drogue_device_kernel::{
    actor::{Actor, Address},
    util::ImmediateFuture,
};
use embassy::time;

pub struct Timer<'a, A: Actor + 'a> {
    _marker: core::marker::PhantomData<&'a A>,
}

pub enum TimerMessage<'m, A: Actor + 'm> {
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
    type OnStartFuture<'m> where 'a: 'm = ImmediateFuture;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;

    fn on_start(self: Pin<&mut Self>) -> Self::OnStartFuture<'_> {
        ImmediateFuture::new()
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match message {
                TimerMessage::Delay(dur) => {
                    time::Timer::after(*dur).await;
                }
                TimerMessage::Schedule(dur, address, message) => {
                    time::Timer::after(*dur).await;
                    address.notify(message.take().unwrap()).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use drogue_device::{testutil::*, *};

    #[derive(Device)]
    struct ScheduleDevice {
        handler: ActorState<'static, TestHandler>,
        timer: ActorState<'static, Timer<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_schedule(mut context: TestContext<ScheduleDevice>) {
        let notified = context.signal();
        context.configure(ScheduleDevice {
            handler: ActorState::new(TestHandler::new(notified)),
            timer: ActorState::new(Timer::new()),
        });

        let (timer_addr, handler_addr) = context.mount(|device| {
            let handler_addr = device.handler.mount(());
            (device.timer.mount(()), handler_addr)
        });

        let before = time::Instant::now();
        timer_addr
            .notify(TimerMessage::schedule(
                time::Duration::from_secs(1),
                handler_addr,
                TestMessage(1),
            ))
            .await;
        notified.wait_signaled().await;
        let after = time::Instant::now();
        assert!(after.as_secs() >= before.as_secs() + 1);
        assert_eq!(1, notified.message().unwrap().0);
    }

    #[derive(Device)]
    struct DelayDevice {
        timer: ActorState<'static, Timer<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_delay(mut context: TestContext<DelayDevice>) {
        context.configure(DelayDevice {
            timer: ActorState::new(Timer::new()),
        });

        let timer = context.mount(|device| device.timer.mount(()));

        let before = time::Instant::now();
        timer
            .process(&mut TimerMessage::Delay(time::Duration::from_secs(1)))
            .await;
        let after = time::Instant::now();
        assert!(after.as_secs() >= before.as_secs() + 1);
    }
}
