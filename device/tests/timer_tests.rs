#![macro_use]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    extern crate std;
    use drogue_device::{testutil::*, *, actors::timer::*};

    #[derive(Device)]
    struct ScheduleDevice {
        handler: ActorContext<'static, TestHandler>,
        timer: ActorContext<'static, Timer<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_schedule(mut context: TestContext<ScheduleDevice>) {
        let notified = context.signal();
        context.configure(ScheduleDevice {
            handler: ActorContext::new(TestHandler::new(notified)),
            timer: ActorContext::new(Timer::new()),
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
        timer: ActorContext<'static, Timer<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_delay(mut context: TestContext<DelayDevice>) {
        context.configure(DelayDevice {
            timer: ActorContext::new(Timer::new()),
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
