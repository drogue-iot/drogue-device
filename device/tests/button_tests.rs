#![macro_use]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    use drogue_device::{actors::button::*, testutil::*, *};

    #[derive(Device)]
    struct TestDevicePressed {
        handler: ActorContext<'static, TestHandler>,
        button: ActorContext<'static, Button<'static, TestPin, TestHandler>>,
    }

    #[drogue::test]
    async fn test_pressed(mut context: TestContext<TestDevicePressed>) {
        let pin = context.pin(true);
        let notified = context.signal();

        context.configure(TestDevicePressed {
            handler: ActorContext::new(TestHandler::new(notified)),
            button: ActorContext::new(Button::new(pin)),
        });

        context.mount(|device| {
            let handler_addr = device.handler.mount(());
            device.button.mount(handler_addr);
        });

        assert!(notified.message().is_none());
        pin.set_low();
        notified.wait_signaled().await;
        assert_eq!(0, notified.message().unwrap().0);
    }

    #[derive(Device)]
    struct TestDeviceReleased {
        handler: ActorContext<'static, TestHandler>,
        button: ActorContext<'static, Button<'static, TestPin, TestHandler>>,
    }

    #[drogue::test]
    async fn test_released(mut context: TestContext<TestDeviceReleased>) {
        let pin = context.pin(false);
        let notified = context.signal();

        context.configure(TestDeviceReleased {
            handler: ActorContext::new(TestHandler::new(notified)),
            button: ActorContext::new(Button::new(pin)),
        });

        context.mount(|device| {
            let handler_addr = device.handler.mount(());
            device.button.mount(handler_addr);
        });

        assert!(notified.message().is_none());
        pin.set_high();
        notified.wait_signaled().await;
        assert_eq!(1, notified.message().unwrap().0);
    }
}
