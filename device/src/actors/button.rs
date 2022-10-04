use crate::traits;
use core::{convert::TryFrom, future::Future};
use ector::{Actor, Address, Inbox};

pub use crate::traits::button::Event as ButtonEvent;

//pub struct Button<P: Wait + InputPin, H: ButtonEventHandler> {
pub struct Button<P: traits::button::Button, H: 'static> {
    inner: P,
    handler: Address<H>,
}

//impl<P: Wait + InputPin, H: ButtonEventHandler> Button<P, H> {
impl<P: traits::button::Button, H> Button<P, H>
where
    H: 'static,
{
    pub fn new(inner: P, handler: Address<H>) -> Self {
        Self { inner, handler }
    }
}

impl<P: traits::button::Button, H> Actor for Button<P, H>
where
    H: TryFrom<ButtonEvent> + 'static,
{
    type Message<'m> = () where Self: 'm;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where Self: 'm, M: Inbox<()> + 'm;
    fn on_mount<'m, M>(&'m mut self, _: Address<()>, _: M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<()> + 'm,
    {
        async move {
            loop {
                let event = self.inner.wait_any().await;
                if let Ok(e) = H::try_from(event) {
                    let _ = self.handler.try_notify(e);
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl TryFrom<ButtonEvent> for ector::testutil::TestMessage {
    type Error = core::convert::Infallible;
    fn try_from(event: ButtonEvent) -> Result<Self, Self::Error> {
        Ok(match event {
            ButtonEvent::Pressed => ector::testutil::TestMessage(0),
            ButtonEvent::Released => ector::testutil::TestMessage(1),
        })
    }
}
