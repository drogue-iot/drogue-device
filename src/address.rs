use crate::actor::{Actor, ActorContext};
use crate::bind::Bind;
use crate::device::Device;
use crate::handler::{NotificationHandler, RequestHandler};
use crate::interrupt::{Interrupt, InterruptContext};
use crate::sink::Sink;
use core::cell::UnsafeCell;

pub struct Address<D: Device, A: Actor<D>> {
    actor: UnsafeCell<*const ActorContext<D, A>>,
}

impl<D: Device, A: Actor<D>> Clone for Address<D, A> {
    fn clone(&self) -> Self {
        Self {
            actor: unsafe { UnsafeCell::new(*self.actor.get()) },
        }
    }
}

// TODO critical sections around ask/tell
impl<D: Device + 'static, A: Actor<D>> Address<D, A> {
    pub(crate) fn new(actor: &ActorContext<D, A>) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
        }
    }

    pub fn bind<OA: Actor<D>>(&self, address: &Address<D, OA>)
    where
        A: Bind<D, OA> + 'static,
        OA: 'static,
    {
        unsafe {
            (&**self.actor.get()).bind(address);
        }
    }

    /*
    pub fn subscribe<OA: Actor>(&self, address: &Address<OA>)
    where
        A: Sink<OA::Event> + 'static,
        OA: 'static,
    {
        unsafe {
            let source = &**address.actor.get();
            let sink = &**self.actor.get();
            source.broker.subscribe(&*sink.actor.get());
        }
    }*/

    pub fn notify<M>(&self, message: M)
    where
        A: NotificationHandler<M> + 'static,
        M: 'static,
    {
        unsafe {
            (&**self.actor.get()).notify(message);
        }
    }

    pub async fn request<M>(&self, message: M) -> <A as RequestHandler<D, M>>::Response
    where
        A: RequestHandler<D, M> + 'static,
        M: 'static,
    {
        unsafe { (&**self.actor.get()).request(message).await }
    }
}

impl<D: Device + 'static, A: Actor<D> + 'static, M: 'static> Sink<M> for Address<D, A>
where
    A: NotificationHandler<M>,
{
    fn notify(&self, message: M) {
        Address::notify(self, message)
    }
}
