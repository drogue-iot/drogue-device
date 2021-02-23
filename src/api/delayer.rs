use crate::domain::time::duration::{Duration, Milliseconds};
use crate::prelude::*;

#[derive(Copy, Clone)]
pub struct Delay<DUR: Duration + Into<Milliseconds>>(pub DUR);

pub trait Delayer: Actor {
    fn delay<DUR>(self, delay: Delay<DUR>) -> Response<Self, ()>
    where
        DUR: Duration + Into<Milliseconds> + 'static;
}

impl<D, DUR> RequestHandler<Delay<DUR>> for D
where
    D: Delayer + Actor + 'static,
    DUR: Duration + Into<Milliseconds> + 'static,
{
    type Response = ();

    fn on_request(self, message: Delay<DUR>) -> Response<Self, Self::Response> {
        self.delay(message)
    }
}

impl<D: Delayer> Address<D> {
    pub async fn delay<DUR>(&self, delay: DUR)
    where
        DUR: Duration + Into<Milliseconds> + 'static,
    {
        self.request(Delay(delay)).await
    }
}
