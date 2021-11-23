use crate::kernel::actor::{Actor, Address, Inbox};
use core::future::Future;
use embassy::traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub enum ButtonEvent {
    Pressed,
    Released,
}

pub struct ButtonEventDispatcher<A: 'static + Actor + FromButtonEvent<A::Message<'static>>> {
    address: Address<'static, A>,
}

impl<A: Actor + FromButtonEvent<A::Message<'static>>> ButtonEventHandler
    for ButtonEventDispatcher<A>
{
    fn handle(&mut self, event: ButtonEvent) {
        if let Some(m) = A::from(event) {
            let _ = self.address.notify(m);
        }
    }
}

impl<A: Actor + FromButtonEvent<A::Message<'static>>> Into<ButtonEventDispatcher<A>>
    for Address<'static, A>
{
    fn into(self) -> ButtonEventDispatcher<A> {
        ButtonEventDispatcher { address: self }
    }
}

pub struct Button<P: WaitForAnyEdge + InputPin, H: ButtonEventHandler> {
    pin: P,
    handler: Option<H>,
}

impl<P: WaitForAnyEdge + InputPin, H: ButtonEventHandler> Button<P, H> {
    pub fn new(pin: P) -> Self {
        Self { pin, handler: None }
    }
}

pub trait ButtonEventHandler {
    fn handle(&mut self, event: ButtonEvent);
}

pub trait FromButtonEvent<M> {
    fn from(event: ButtonEvent) -> Option<M>
    where
        Self: Sized;
}

impl<P: WaitForAnyEdge + InputPin, H: ButtonEventHandler> Actor for Button<P, H> {
    type Configuration = H;

    type OnMountFuture<'m, M>
    where
        M: 'm,
        H: 'm,
        P: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
        Self: 'm,
    {
        self.handler.replace(config);
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                let event = if self.pin.is_high().ok().unwrap() {
                    trace!("Button released");
                    ButtonEvent::Released
                } else {
                    trace!("Button pressed");
                    ButtonEvent::Pressed
                };

                if let Some(handler) = &mut self.handler {
                    handler.handle(event);
                }
            }
        }
    }
}
