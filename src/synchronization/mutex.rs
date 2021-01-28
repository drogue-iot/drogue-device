use crate::actor::Actor;
use crate::address::Address;
use crate::bus::EventBus;
use crate::device::{Device, Lifecycle};
use crate::handler::{Completion, NotificationHandler, RequestHandler, Response};
use core::cell::UnsafeCell;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use heapless::{consts::*, spsc::Queue};

pub struct Lock;

pub struct Unlock<T>(T);

pub struct Mutex<D, T>
    where
        D: Device,
        T: 'static
{
    address: Option<Address<D, Self>>,
    val: Option<T>,
    waiters: Queue<Waker, U16>,
}

impl<D, T> Actor<D> for Mutex<D, T>
    where
        D: Device,
        T: 'static
{
    fn mount(&mut self, addr: Address<D, Self>, _: EventBus<D>) {
        self.address.replace(addr);
    }
}

impl<D, T> RequestHandler<D, Lock>
for Mutex<D, T>
    where
        D: Device + 'static,
        T: 'static
{
    type Response = Exclusive<D, T>;

    fn on_request(&'static mut self, message: Lock) -> Response<Self::Response> {
        Response::defer(async move {
            let lock = Exclusive {
                address: self.address.as_ref().unwrap().clone(),
                val: Some(self.lock().await),
            };
            log::trace!("[Mutex<T> lock");
            lock
        })
    }
}

impl<D, T> NotificationHandler<Lifecycle>
for Mutex<D, T>
    where
        D: Device,
        T: 'static
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D, T> NotificationHandler<Unlock<T>>
for Mutex<D, T>
    where
        D: Device,
        T: 'static
{
    fn on_notification(&'static mut self, message: Unlock<T>) -> Completion {
        log::trace!("[Mutex<T> unlock");
        self.unlock(message.0);
        Completion::immediate()
    }
}

impl<D, T> Mutex<D, T>
    where D: Device
{
    pub fn new(val: T) -> Self {
        Self {
            address: None,
            val: Some(val),
            waiters: Queue::new(),
        }
    }

    pub async fn lock(&'static mut self) -> T {
        struct LockFuture<D: Device, TT: 'static> {
            waiting: bool,
            mutex: UnsafeCell<*mut Mutex<D, TT>>,
        }

        impl<D: Device, TT> Future for LockFuture<D, TT> {
            type Output = TT;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                unsafe {
                    if let Some(val) = (**self.mutex.get()).val.take() {
                        Poll::Ready(val)
                    } else {
                        if !self.waiting {
                            self.waiting = true;
                            (**self.mutex.get())
                                .waiters
                                .enqueue(cx.waker().clone())
                                .unwrap_or_else(|_| panic!("too many waiters"))
                        }
                        Poll::Pending
                    }
                }
            }
        }

        LockFuture {
            waiting: false,
            mutex: UnsafeCell::new(self),
        }
            .await
    }

    pub fn unlock(&mut self, val: T) {
        self.val.replace(val);
        if let Some(next) = self.waiters.dequeue() {
            next.wake()
        }
    }
}

pub struct Exclusive<D, T>
    where
        D: Device + 'static,
        T: 'static
{
    val: Option<T>,
    address: Address<D, Mutex<D, T>>,
}

impl<D, T> Deref
for Exclusive<D, T>
    where D: Device
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.val.as_ref().unwrap()
    }
}

impl<D, T> DerefMut
for Exclusive<D, T>
    where
        D: Device
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.val.as_mut().unwrap()
    }
}

impl<D, T> Drop for Exclusive<D, T>
    where
        D: Device + 'static,
        T: 'static
{
    fn drop(&mut self) {
        self.address.notify(Unlock(self.val.take().unwrap()))
    }
}

impl<D, T> Address<D, Mutex<D, T>>
    where D: Device
{
    pub async fn lock(&'static self) -> Exclusive<D, T> {
        self.request(Lock).await
    }
}
