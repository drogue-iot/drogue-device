use core::future::Future;
use core::pin::Pin;
use drogue_device_kernel::{
    actor::{Actor, Address},
    channel::consts,
    util::ImmediateFuture,
};
use embassy_traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub trait FromButtonEvent<M> {
    fn from(event: ButtonEvent) -> Option<M>
    where
        Self: Sized;
}

pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct Button<
    'a,
    P: WaitForAnyEdge + InputPin + 'a,
    A: Actor + FromButtonEvent<A::Message<'a>> + 'a,
> {
    pin: P,
    handler: Option<Address<'a, A>>,
}

impl<'a, P: WaitForAnyEdge + InputPin + 'a, A: Actor + FromButtonEvent<A::Message<'a>> + 'a>
    Button<'a, P, A>
{
    pub fn new(pin: P) -> Self {
        Self { pin, handler: None }
    }
}

impl<'a, P: WaitForAnyEdge + InputPin + 'a, A: Actor + FromButtonEvent<A::Message<'a>> + 'a> Unpin
    for Button<'a, P, A>
{
}

impl<'a, P: WaitForAnyEdge + InputPin + 'a, A: Actor + FromButtonEvent<A::Message<'a>> + 'a> Actor
    for Button<'a, P, A>
{
    type MaxQueueSize<'m>
    where
        'a: 'm,
    = consts::U0;
    type Configuration = Address<'a, A>;
    #[rustfmt::skip]
    type Message<'m> where 'a: 'm = ();
    #[rustfmt::skip]
    type OnStartFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where 'a: 'm = ImmediateFuture;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.handler.replace(config);
    }

    fn on_start(mut self: Pin<&mut Self>) -> Self::OnStartFuture<'_> {
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                let event = if self.pin.is_high().ok().unwrap() {
                    ButtonEvent::Released
                } else {
                    ButtonEvent::Pressed
                };

                if let Some(handler) = self.handler {
                    let mut message = A::from(event);
                    if let Some(m) = message.take() {
                        handler.send(m).await;
                    }
                }
            }
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        _: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        ImmediateFuture::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drogue_device::{testutil::*, *};

    impl FromButtonEvent<TestMessage> for TestHandler {
        fn from(event: ButtonEvent) -> Option<TestMessage> {
            match event {
                ButtonEvent::Pressed => Some(TestMessage(0)),
                ButtonEvent::Released => Some(TestMessage(1)),
            }
        }
    }

    #[test]
    fn test_pressed() {
        #[derive(drogue::Device)]
        struct TestDevice {
            handler: ActorState<'static, TestHandler>,
            button: ActorState<'static, Button<'static, TestPin, TestHandler>>,
        }

        let context = TestContext::new();
        let pin = context.pin(true);
        let notified = context.signal();

        context.configure(TestDevice {
            handler: ActorState::new(TestHandler::new(notified)),
            button: ActorState::new(Button::new(pin)),
        });

        context.mount(|device| {
            let handler_addr = device.handler.mount(());
            device.button.mount(handler_addr);
        });

        assert!(!notified.signaled());
        pin.set_low();
        context.run_until_idle();
        assert!(notified.signaled());
        assert_eq!(0, notified.message().unwrap().0);
    }

    #[test]
    fn test_released() {
        #[derive(drogue::Device)]
        struct TestDevice {
            handler: ActorState<'static, TestHandler>,
            button: ActorState<'static, Button<'static, TestPin, TestHandler>>,
        }

        let context = TestContext::new();
        let pin = context.pin(false);
        let notified = context.signal();

        context.configure(TestDevice {
            handler: ActorState::new(TestHandler::new(notified)),
            button: ActorState::new(Button::new(pin)),
        });

        context.mount(|device| {
            let handler_addr = device.handler.mount(());
            device.button.mount(handler_addr);
        });

        assert!(!notified.signaled());
        pin.set_high();
        context.run_until_idle();
        assert!(notified.signaled());
        assert_eq!(1, notified.message().unwrap().0);
    }
}
