use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use embassy::executor::Spawner;

pub struct ImmediateFuture {}

impl Future for ImmediateFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

pub trait Package {
    fn start(&'static self, spawner: Spawner) -> ImmediateFuture;
}
