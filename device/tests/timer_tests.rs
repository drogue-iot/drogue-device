#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(test)]
mod tests {
    extern crate std;
    use drogue_device::{actors::timer::*, testutil::*, *};
    use drogue_device_macros::test as drogue_test;
    use embassy::executor::Spawner;
    use embassy::time;

    struct ScheduleDevice {
        handler: ActorContext<'static, TestHandler>,
        timer: ActorContext<'static, Timer<'static, TestHandler>>,
    }

    #[drogue_test]
    async fn test_schedule(spawner: Spawner, mut context: TestContext<ScheduleDevice>) {
        let notified = context.signal();
        context.configure(ScheduleDevice {
            handler: ActorContext::new(TestHandler::new(notified)),
            timer: ActorContext::new(Timer::new()),
        });

        let (timer_addr, handler_addr) = context
            .mount(|device| async move {
                let handler_addr = device.handler.mount((), spawner);
                (device.timer.mount((), spawner), handler_addr)
            })
            .await;

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

    #[drogue_test]
    async fn test_delay(spawner: Spawner, mut context: TestContext<DelayDevice>) {
        context.configure(DelayDevice {
            timer: ActorContext::new(Timer::new()),
        });

        let timer = context
            .mount(|device| async move { device.timer.mount((), spawner) })
            .await;

        let before = time::Instant::now();
        timer
            .request(TimerMessage::Delay(time::Duration::from_secs(1)))
            .unwrap()
            .await;
        let after = time::Instant::now();
        assert!(after.as_secs() >= before.as_secs() + 1);
    }
}
