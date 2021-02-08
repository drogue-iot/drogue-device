//! A mutex-lock actor and supporting types.

use crate::prelude::*;
use core::cell::RefCell;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use heapless::{consts::*, spsc::Queue};

/// The lock request message.
#[derive(Debug)]
pub struct Lock;

#[doc(hidden)]
pub struct Unlock<T>(T);

/// A Mutex lock actor.
///
/// `<T>` is the type of object protected by the mutex.
///
/// The `Address<Mutex<T>>` provides an asynchronous `lock()` method
/// which can be used to `.await` exclusive access to the underlying resource.
///
/// The result is an `Exclusive<T>` which provides exclusive mutable access
/// to the underlying resource until dropped, at which point the lock will be
/// released.
///

pub struct Shared<T> {
    val: RefCell<Option<T>>,
    waiters: RefCell<Queue<Waker, U16>>,
}

impl<T> Shared<T> {
    fn new(val: T) -> Self {
        Self {
            val: RefCell::new(Some(val)),
            waiters: RefCell::new(Queue::new()),
        }
    }

    fn lock(&self) -> Option<T> {
        self.val.borrow_mut().take()
    }

    fn unlock(&self, val: T) {
        self.val.borrow_mut().replace(val);
        if let Some(next) = self.waiters.borrow_mut().dequeue() {
            next.wake()
        }
    }

    fn waiting(&self, waker: &Waker) {
        self.waiters.borrow_mut().enqueue(waker.clone()).ok();
    }
}

pub struct Mutex<T: 'static> {
    shared: Shared<T>,
    actor: ActorContext<MutexActor<T>>,
}

impl<T: 'static> Mutex<T> {
    pub fn new(val: T) -> Self {
        Self {
            shared: Shared::new(val),
            actor: ActorContext::new(MutexActor::new()),
        }
    }

    pub fn configure(&self, config: &'static T::Configuration)
    where
        T: Configurable,
    {
        self.shared
            .val
            .borrow_mut()
            .as_mut()
            .unwrap()
            .configure(config)
    }
}

impl<D: Device, T: 'static> Package<D, MutexActor<T>> for Mutex<T> {
    fn mount(
        &'static self,
        bus_address: Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<MutexActor<T>> {
        let addr = self.actor.mount(supervisor);
        self.actor.configure(&self.shared);
        addr
    }
}

pub struct MutexActor<T>
where
    T: 'static,
{
    shared: Option<&'static Shared<T>>,
    address: Option<Address<Self>>,
}

impl<T> Actor for MutexActor<T>
where
    T: 'static,
{
    fn on_mount(&mut self, addr: Address<Self>) {
        self.address.replace(addr);
    }
}

impl<T> Configurable for MutexActor<T> {
    type Configuration = Shared<T>;

    fn configure(&mut self, config: &'static Self::Configuration) {
        self.shared.replace(config);
    }
}

impl<T, A: Actor> Bind<A> for Mutex<T>
where
    T: Bind<A>,
{
    fn on_bind(&mut self, address: Address<A>) {
        self.shared
            .val
            .borrow_mut()
            .as_mut()
            .unwrap()
            .on_bind(address);
    }
}

impl<T> RequestHandler<Lock> for MutexActor<T>
where
    T: 'static,
{
    type Response = Exclusive<T>;

    fn on_request(mut self, message: Lock) -> Response<Self, Self::Response> {
        Response::defer(async move {
            let lock = Exclusive {
                address: self.address.unwrap(),
                val: Some(self.lock().await),
            };
            log::trace!("[Mutex<T> lock");
            self.respond_with(lock)
        })
    }
}

impl<T> NotifyHandler<Unlock<T>> for MutexActor<T>
where
    T: 'static,
{
    fn on_notify(mut self, message: Unlock<T>) -> Completion<Self> {
        log::trace!("[Mutex<T> unlock");
        self.unlock(message.0);
        Completion::immediate(self)
    }
}

impl<T> MutexActor<T> {
    fn new() -> Self {
        Self {
            address: None,
            shared: None,
        }
    }

    #[doc(hidden)]
    pub async fn lock(&mut self) -> T {
        struct LockFuture<TT: 'static> {
            waiting: bool,
            shared: &'static Shared<TT>,
            address: Address<MutexActor<TT>>,
        }

        impl<TT> Future for LockFuture<TT> {
            type Output = TT;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if let Some(val) = self.shared.lock() {
                    return Poll::Ready(val);
                }
                if !self.waiting {
                    self.shared.waiting(cx.waker());
                    self.waiting = true;
                }

                Poll::Pending
            }
        }

        LockFuture {
            waiting: false,
            shared: self.shared.unwrap(),
            address: self.address.unwrap(),
        }
        .await
    }

    #[doc(hidden)]
    pub fn unlock(&mut self, val: T) {
        self.shared.unwrap().unlock(val);
    }
}

/// A scope-limited exclusive reference to the underlying lockable resource.
///
/// When the exclusive instance is dropped, the lock will be returned to the
/// mutex and the next waiter, if any, will be provide the resource.
pub struct Exclusive<T>
where
    T: 'static,
{
    val: Option<T>,
    address: Address<MutexActor<T>>,
}

impl<T> Deref for Exclusive<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.val.as_ref().unwrap()
    }
}

impl<T> DerefMut for Exclusive<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.val.as_mut().unwrap()
    }
}

impl<T> Drop for Exclusive<T>
where
    T: 'static,
{
    fn drop(&mut self) {
        self.address.notify(Unlock(self.val.take().unwrap()))
    }
}

impl<T> Address<MutexActor<T>> {
    pub async fn lock(&self) -> Exclusive<T> {
        self.request(Lock).await
    }
}
