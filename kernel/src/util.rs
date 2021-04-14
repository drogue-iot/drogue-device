use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct ImmediateFuture;

impl ImmediateFuture {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ImmediateFuture {
    fn default() -> ImmediateFuture {
        ImmediateFuture::new()
    }
}

impl Future for ImmediateFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}
