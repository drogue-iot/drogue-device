#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    use drogue_device::{actors::button::*, testutil::*, *};
    use drogue_device_macros::test as drogue_test;
    use embassy::executor::Spawner;

    struct TestDevicePressed {
        handler: ActorContext<'static, TestHandler>,
        button: ActorContext<'static, Button<'static, TestPin, TestHandler>>,
    }

    //TODO: Broken
    //#[drogue_test]
    async fn test_pressed(spawner: Spawner, mut context: TestContext<TestDevicePressed>) {
        let pin = context.pin(true);
        let notified = context.signal();

        context.configure(TestDevicePressed {
            handler: ActorContext::new(TestHandler::new(notified)),
            button: ActorContext::new(Button::new(pin)),
        });

        context
            .mount(|device| async move {
                let handler_addr = device.handler.mount((), spawner);
                device.button.mount(handler_addr, spawner);
            })
            .await;

        assert!(notified.message().is_none());
        pin.set_low();
        notified.wait_signaled().await;
        assert_eq!(0, notified.message().unwrap().0);
    }

    struct TestDeviceReleased {
        handler: ActorContext<'static, TestHandler>,
        button: ActorContext<'static, Button<'static, TestPin, TestHandler>>,
    }

    // TODO: Broken
    //#[drogue_test]
    async fn test_released(spawner: Spawner, mut context: TestContext<TestDeviceReleased>) {
        let pin = context.pin(false);
        let notified = context.signal();

        context.configure(TestDeviceReleased {
            handler: ActorContext::new(TestHandler::new(notified)),
            button: ActorContext::new(Button::new(pin)),
        });

        context
            .mount(|device| async move {
                let handler_addr = device.handler.mount((), spawner);
                device.button.mount(handler_addr, spawner);
            })
            .await;

        assert!(notified.message().is_none());
        pin.set_high();
        notified.wait_signaled().await;
        assert_eq!(1, notified.message().unwrap().0);
    }
}
