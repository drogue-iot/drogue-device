use crate::kernel::actor::{Actor, Address, Inbox};
use crate::traits;
use core::future::Future;

pub use crate::traits::button::Event as ButtonEvent;

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

//pub struct Button<P: WaitForAnyEdge + InputPin, H: ButtonEventHandler> {
pub struct Button<P: traits::button::Button, H: ButtonEventHandler> {
    inner: P,
    handler: Option<H>,
}

//impl<P: WaitForAnyEdge + InputPin, H: ButtonEventHandler> Button<P, H> {
impl<P: traits::button::Button, H: ButtonEventHandler> Button<P, H> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            handler: None,
        }
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

impl<P: traits::button::Button, H: ButtonEventHandler> Actor for Button<P, H> {
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
                let event = self.inner.wait_any().await;
                if let Some(handler) = &mut self.handler {
                    handler.handle(event);
                }
            }
        }
    }
}
