use core::future::Future;
use crate::actor::Actor;
use crate::alloc::{Box, alloc};

pub enum Response<T> {
    Immediate(T),
    Defer(Box<dyn Future<Output=T>>),
}

impl<T> Response<T> {
    pub fn immediate(val: T) -> Self {
        Self::Immediate(val)
    }

    pub fn defer<F: Future<Output=T> + 'static>(f: F) -> Self
        where T: 'static
    {
        Self::Defer(
            Box::new(alloc(f).unwrap())
        )
    }
}

pub trait AskHandler<M>
    where Self: Actor + Sized
{
    type Response: 'static;

    fn on_message(&'static mut self, message: M) -> Response<Self::Response>;
}

pub enum Completion {
    Immediate(),
    Defer(Box<dyn Future<Output=()>>)
}

impl Completion {
    pub fn immediate() -> Self {
        Self::Immediate()
    }

    pub fn defer<F: Future<Output=()> + 'static>(f: F) -> Self {
        Self::Defer(
            Box::new( alloc( f ).unwrap() )
        )
    }

}

pub trait TellHandler<M>
    where Self: Actor + Sized
{
    fn on_message(&'static mut self, message: M) -> Completion;
}

