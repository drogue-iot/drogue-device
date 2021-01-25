use crate::actor::{Actor, ActorContext};
use crate::bind::Bind;
use crate::handler::{NotificationHandler, RequestHandler};
use crate::interrupt::{Interrupt, InterruptContext};
use crate::sink::Sink;
use core::cell::UnsafeCell;

pub struct Address<A: Actor> {
    actor: UnsafeCell<*const ActorContext<A>>,
}

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self {
            actor: unsafe { UnsafeCell::new(*self.actor.get()) },
        }
    }
}

// TODO critical sections around ask/tell
impl<A: Actor> Address<A> {
    pub(crate) fn new(actor: &ActorContext<A>) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
        }
    }

    pub fn bind<OA: Actor>(&self, address: &Address<OA>)
    where
        A: Bind<OA> + 'static,
        OA: 'static,
    {
        unsafe {
            (&**self.actor.get()).bind(address);
        }
    }

    pub fn subscribe<OA: Actor>(&self, actor: &ActorContext<OA>)
    where
        A: 'static,
        OA: Sink<A::Event> + 'static,
    {
        unsafe {
            let a = &*actor.actor.get();
            (&**self.actor.get()).broker.subscribe(a);
        }
    }

    pub fn notify<M>(&self, message: M)
    where
        A: NotificationHandler<M> + 'static,
        M: 'static,
    {
        unsafe {
            (&**self.actor.get()).notify(message);
        }
    }

    pub async fn request<M>(&self, message: M) -> <A as RequestHandler<M>>::Response
    where
        A: RequestHandler<M> + 'static,
        M: 'static,
    {
        unsafe { (&**self.actor.get()).request(message).await }
    }
}

impl<A: Actor + 'static, M: 'static> Sink<M> for Address<A>
where
    A: NotificationHandler<M>,
{
    fn notify(&self, message: M) {
        Address::notify(self, message)
    }
}
