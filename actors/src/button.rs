use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use drogue_device_kernel::{
    actor::{Actor, Address},
    util::ImmediateFuture,
};
use embassy_traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub trait FromButtonEvent {
    fn from(event: ButtonEvent) -> Option<Self>
    where
        Self: Sized;
}

pub enum ButtonEvent {
    Pressed,
    Released,
}

#[rustfmt::skip]
pub struct Button<
    P: WaitForAnyEdge + InputPin + 'static,
    M: FromButtonEvent + 'static,
    A: Actor<Message<'static> = M> + 'static,
> {
    pin: P,
    handler: Option<Address<'static, A>>,
    _phantom: PhantomData<M>,
}

#[rustfmt::skip]
impl<
        P: WaitForAnyEdge + InputPin + 'static,
        M: FromButtonEvent + 'static,
        A: Actor<Message<'static> = M> + 'static,
    > Button<P, M, A>
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            handler: None,
            _phantom: PhantomData,
        }
    }
}

#[rustfmt::skip]
impl<
        P: WaitForAnyEdge + InputPin + 'static,
        M: FromButtonEvent + 'static,
        A: Actor<Message<'static> = M> + 'static,
    > Unpin for Button<P, M, A>
{
}

#[rustfmt::skip]
impl<
        P: WaitForAnyEdge + InputPin + 'static,
        M: FromButtonEvent + 'static,
        A: Actor<Message<'static> = M> + 'static,
    > Actor for Button<P, M, A>
{
    type Configuration = Address<'static, A>;
    type Message<'a> = ();
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = ImmediateFuture;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.handler.replace(config);
    }

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                let event = if self.pin.is_high().ok().unwrap() {
                    ButtonEvent::Released
                } else {
                    ButtonEvent::Pressed
                };

                if let Some(handler) = self.handler {
                    let mut message: Option<M> = M::from(event);
                    if let Some(m) = message.take() {
                        handler
                            .send(m)
                            .await;
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
