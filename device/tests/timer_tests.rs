#![macro_use]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(test)]
mod tests {
    extern crate std;
    use drogue_device::{actors::timer::*, testutil::*, *};

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

        let (timer_addr, handler_addr) = context.mount(|device, spawner| {
            let handler_addr = device.handler.mount((), spawner);
            (device.timer.mount((), spawner), handler_addr)
        });

        let before = time::Instant::now();
        timer_addr
            .notify(TimerMessage::schedule(
                time::Duration::from_secs(1),
                handler_addr,
                TestMessage(1),
            ))
            .unwrap();
        notified.wait_signaled().await;
        let after = time::Instant::now();
        assert!(after.as_secs() >= before.as_secs() + 1);
        assert_eq!(1, notified.message().unwrap().0);
    }

    struct DelayDevice {
        timer: ActorContext<'static, Timer<'static, TestHandler>>,
    }

    #[drogue::test]
    async fn test_delay(mut context: TestContext<DelayDevice>) {
        context.configure(DelayDevice {
            timer: ActorContext::new(Timer::new()),
        });

        let timer = context.mount(|device, spawner| device.timer.mount((), spawner));

        let before = time::Instant::now();
        timer
            .request(TimerMessage::Delay(time::Duration::from_secs(1)))
            .unwrap()
            .await;
        let after = time::Instant::now();
        assert!(after.as_secs() >= before.as_secs() + 1);
    }
}
