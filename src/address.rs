use crate::actor::{Actor, ActorContext};
use crate::bind::Bind;
use crate::handler::{NotificationHandler, RequestHandler};
use crate::interrupt::{Interrupt, InterruptContext};
use crate::sink::{Message, Sink};
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

    pub fn subscribe<I: Interrupt>(&self, address: &InterruptAddress<I>)
    where
        I: 'static,
        A: Sink<I::Event> + 'static,
    {
        unsafe {
            let source = &**address.actor.get();
            let sink = &**self.actor.get();
            source.add_subscriber(&*sink.actor.get());
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

/// An interrupt address wraps an address providing interrupt context.

pub struct InterruptAddress<I: Interrupt> {
    address: Address<I>,
    actor: UnsafeCell<*const InterruptContext<I>>,
}

// TODO critical sections around ask/tell
impl<I: Interrupt> InterruptAddress<I> {
    pub(crate) fn new(actor: &InterruptContext<I>, address: Address<I>) -> Self {
        Self {
            address,
            actor: UnsafeCell::new(actor),
        }
    }

    pub fn bind<OA: Actor>(&self, address: &Address<OA>)
    where
        I: Bind<OA> + 'static,
        OA: 'static,
    {
        self.address.bind(address);
    }

    pub fn notify<M>(&self, message: M)
    where
        I: NotificationHandler<M> + 'static,
        M: 'static,
    {
        self.address.notify(message);
    }

    pub async fn request<M>(&self, message: M) -> <I as RequestHandler<M>>::Response
    where
        I: RequestHandler<M> + 'static,
        M: 'static,
    {
        self.address.request(message).await
    }
}

impl<I: Interrupt + 'static, M: 'static> Sink<M> for InterruptAddress<I>
where
    M: Message,
    I: NotificationHandler<M>,
{
    fn notify(&self, message: M) {
        Address::notify(&self.address, message)
    }
}
