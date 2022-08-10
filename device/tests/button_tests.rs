#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    use drogue_device::actors::button::*;
    #[allow(unused_imports)]
    use drogue_device_macros::test as drogue_test;
    use ector::testutil::*;
    use embassy_executor::executor::Spawner;

    #[allow(dead_code)]
    struct TestDevicePressed {
        handler: ector::ActorContext<TestHandler>,
        button: ector::ActorContext<Button<TestPin, TestMessage>>,
    }

    #[drogue_test]
    #[allow(dead_code)]
    async fn test_pressed(spawner: Spawner, mut context: TestContext<TestDevicePressed>) {
        let pin = context.pin(true);
        let notified = context.signal();

        let device = context.configure(TestDevicePressed {
            handler: ector::ActorContext::new(),
            button: ector::ActorContext::new(),
        });
        let handler_addr = device.handler.mount(spawner, TestHandler::new(notified));
        device.button.mount(spawner, Button::new(pin, handler_addr));

        assert!(notified.message().is_none());
        pin.set_high();
        notified.wait_signaled().await;
        assert_eq!(0, notified.message().unwrap().0);
    }

    #[allow(dead_code)]
    struct TestDeviceReleased {
        handler: ector::ActorContext<TestHandler>,
        button: ector::ActorContext<Button<TestPin, TestMessage>>,
    }

    #[drogue_test]
    #[allow(dead_code)]
    async fn test_released(spawner: Spawner, mut context: TestContext<TestDeviceReleased>) {
        let pin = context.pin(false);
        let notified = context.signal();

        let device = context.configure(TestDeviceReleased {
            handler: ector::ActorContext::new(),
            button: ector::ActorContext::new(),
        });

        let handler_addr = device.handler.mount(spawner, TestHandler::new(notified));
        device.button.mount(spawner, Button::new(pin, handler_addr));

        assert!(notified.message().is_none());
        pin.set_low();
        notified.wait_signaled().await;
        assert_eq!(1, notified.message().unwrap().0);
    }
}
