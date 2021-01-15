use crate::actor::{Actor, ActorContext, ActorFuture};
use core::marker::PhantomData;
use core::pin::Pin;
use core::cell::UnsafeCell;
use crate::handler::{AskHandler, TellHandler};
use heapless::ArrayLength;

pub struct Address<A: Actor> {
    actor: UnsafeCell<*const ActorContext<A>>,
}

impl<A: Actor> Address<A> {
    pub(crate) fn new(actor: &ActorContext<A>) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
        }
    }

    pub fn tell<M>(&self, message: M)
        where A: TellHandler<M> + 'static,
              M: 'static
    {
        unsafe {
            (&**self.actor.get()).tell(message);
        }
    }

    pub async fn ask<M>(&self, message: M) -> <A as AskHandler<M>>::Response
        where A: AskHandler<M> + 'static,
              M: 'static
    {
        unsafe {
            (&**self.actor.get()).ask(message).await
        }
    }
}