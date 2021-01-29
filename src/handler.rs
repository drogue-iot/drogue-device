use crate::actor::Actor;
use crate::alloc::{alloc, Box};
use crate::device::Device;
use core::future::Future;

pub enum Response<T> {
    Immediate(T),
    Defer(Box<dyn Future<Output = T>>),
    ImmediateFuture(Box<dyn Future<Output = T>>),
}

impl<T> Response<T> {
    pub fn immediate(val: T) -> Self {
        Self::Immediate(val)
    }

    pub fn defer<F: Future<Output = T> + 'static>(f: F) -> Self
    where
        T: 'static,
    {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }

    pub fn immediate_future<F: Future<Output = T> + 'static>(f: F) -> Self
    where
        T: 'static,
    {
        Self::ImmediateFuture(Box::new(alloc(f).unwrap()))
    }
}

pub trait RequestHandler<M>
where
    Self: Actor + Sized,
{
    type Response: 'static;

    fn on_request(&'static mut self, message: M) -> Response<Self::Response>;
}

pub enum Completion {
    Immediate(),
    Defer(Box<dyn Future<Output = ()>>),
}

impl Completion {
    pub fn immediate() -> Self {
        Self::Immediate()
    }

    pub fn defer<F: Future<Output = ()> + 'static>(f: F) -> Self {
        Self::Defer(Box::new(alloc(f).unwrap()))
    }
}

pub trait NotificationHandler<M>
where
    Self: Sized,
{
    fn on_notification(&'static mut self, message: M) -> Completion;
}
