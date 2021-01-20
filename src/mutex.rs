use core::task::{Waker, Context, Poll};
use heapless::{
    spsc::Queue,
    consts::*,
};
use core::cell::UnsafeCell;
use core::future::Future;
use core::pin::Pin;
use crate::actor::Actor;
use crate::address::Address;
use crate::handler::{RequestHandler, Response, NotificationHandler, Completion};
use core::ops::{Deref, DerefMut};

pub struct Lock;

pub struct Unlock<T>(T);

pub struct Mutex<T> {
    address: Option<Address<Self>>,
    val: Option<T>,
    waiters: Queue<Waker, U16>,
}

impl<T> Actor for Mutex<T> {
    fn start(&mut self, addr: Address<Self>) {
        self.address.replace(addr);
    }
}

impl<T: 'static> RequestHandler<Lock> for Mutex<T> {
    type Response = Exclusive<T>;

    fn on_request(&'static mut self, message: Lock) -> Response<Self::Response> {
        Response::defer(async move {
            let lock = Exclusive {
                address: self.address.as_ref().unwrap().clone(),
                val: Some(self.lock().await),
            };
            log::info!("returning Exclusive");
            lock
        })
    }
}

impl<T: 'static> NotificationHandler<Unlock<T>> for Mutex<T> {
    fn on_notification(&'static mut self, message: Unlock<T>) -> Completion {
        self.unlock(message.0);
        Completion::immediate()
    }
}

impl<T> Mutex<T> {
    pub fn new(val: T) -> Self {
        Self {
            address: None,
            val: Some(val),
            waiters: Queue::new(),
        }
    }

    pub async fn lock(&'static mut self) -> T {
        struct LockFuture<TT: 'static> {
            waiting: bool,
            mutex: UnsafeCell<*mut Mutex<TT>>,
        }

        impl<TT> Future for LockFuture<TT> {
            type Output = TT;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                unsafe {
                    log::info!("polling mutex");
                    if let Some(val) = (**self.mutex.get()).val.take() {
                        log::info!("successful mutex lock");
                        Poll::Ready(val)
                    } else {
                        log::info!("waiting mutex lock");
                        if !self.waiting {
                            self.waiting = true;
                            (**self.mutex.get()).waiters.enqueue(cx.waker().clone()).unwrap_or_else(|_| panic!("too many waiters"))
                            //(&mut **self.mutex.get()).waiters.enqueue(cx.waker().clone()).unwrap_or_else(|_| panic!("too many waiters"))
                        }
                        Poll::Pending
                    }
                }
            }
        }

        LockFuture {
            waiting: false,
            mutex: UnsafeCell::new(self),
        }.await
    }

    pub fn unlock(&mut self, val: T) {
        log::info!("unlocking mutex");
        self.val.replace(val);
        if let Some(next) = self.waiters.dequeue() {
            next.wake()
        }
    }
}

pub struct Exclusive<T: 'static> {
    val: Option<T>,
    address: Address<Mutex<T>>,
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

impl<T: 'static> Drop for Exclusive<T> {
    fn drop(&mut self) {
        self.address.notify(Unlock(self.val.take().unwrap()))
    }
}

impl<T> Address<Mutex<T>> {
    pub async fn lock(&'static mut self) -> Exclusive<T> {
        self.request(Lock).await
    }
}