use crate::impl_actor;
use crate::kernel::actor::{Actor, Address, Inbox};
use crate::traits;

pub use crate::traits::button::Event as ButtonEvent;

pub struct ButtonEventDispatcher<A: 'static + Actor + FromButtonEvent<A::Message<'static>>> {
    address: Address<A>,
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
    for Address<A>
{
    fn into(self) -> ButtonEventDispatcher<A> {
        ButtonEventDispatcher { address: self }
    }
}

pub struct ButtonPressed<A>(pub Address<A>, pub A::Message<'static>)
where
    A: Actor + 'static,
    A::Message<'static>: Clone;

impl<A> ButtonEventHandler for ButtonPressed<A>
where
    A: Actor + 'static,
    A::Message<'static>: Clone,
{
    fn handle(&mut self, event: ButtonEvent) {
        if let ButtonEvent::Pressed = event {
            let _ = self.0.notify(self.1.clone());
        }
    }
}

//pub struct Button<P: Wait + InputPin, H: ButtonEventHandler> {
pub struct Button<P: traits::button::Button, H: ButtonEventHandler> {
    inner: P,
    handler: H,
}

//impl<P: Wait + InputPin, H: ButtonEventHandler> Button<P, H> {
impl<P: traits::button::Button, H: ButtonEventHandler> Button<P, H> {
    pub fn new(inner: P, handler: H) -> Self {
        Self { inner, handler }
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

#[impl_actor]
impl<P: traits::button::Button, H: ButtonEventHandler> Actor for Button<P, H> {
    async fn on_mount<M>(&mut self, _: Address<Self>, _: &mut M)
    where
        M: Inbox<Self>,
    {
        loop {
            let event = self.inner.wait_any().await;
            self.handler.handle(event);
        }
    }
}
