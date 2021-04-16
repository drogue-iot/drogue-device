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
    'a,
    P: WaitForAnyEdge + InputPin + 'a,
    M: FromButtonEvent + 'a,
    A: Actor<Message<'a> = M> + 'a,
> {
    pin: P,
    handler: Option<Address<'a, A>>,
    _phantom: PhantomData<M>,
}

#[rustfmt::skip]
impl<
        'a,
        P: WaitForAnyEdge + InputPin + 'a,
        M: FromButtonEvent + 'a,
        A: Actor<Message<'a> = M> + 'a,
    > Button<'a, P, M, A>
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
        'a,
        P: WaitForAnyEdge + InputPin + 'a,
        M: FromButtonEvent + 'a,
        A: Actor<Message<'a> = M> + 'a,
    > Unpin for Button<'a, P, M, A>
{
}

#[rustfmt::skip]
impl<
        'a,
        P: WaitForAnyEdge + InputPin + 'a,
        M: FromButtonEvent + 'a,
        A: Actor<Message<'a> = M> + 'a,
    > Actor for Button<'a, P, M, A>
{
    type Configuration = Address<'a, A>;
    type Message<'m> where 'a: 'm = ();
    type OnStartFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    type OnMessageFuture<'m> where 'a: 'm = ImmediateFuture;

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
