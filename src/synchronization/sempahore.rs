//! A semaphore actor and supporting types.

use crate::prelude::{Actor, Device, NotifyHandler, Address};
use crate::handler::{Completion, RequestHandler, Response};
use crate::bus::EventBus;
use core::future::Future;
use core::task::{Context, Poll, Waker};
use core::pin::Pin;
use core::cell::UnsafeCell;

use heapless::{
    spsc::Queue,
    consts::*,
};

pub struct Acquire;

pub struct Release;

pub struct Semaphore
{
    address: Option<Address<Self>>,
    permits: usize,
    waiters: Queue<Waker, U16>,
}

impl Semaphore
{
    pub fn new(permits: usize) -> Self {
        Self {
            address: None,
            permits,
            waiters: Queue::new(),
        }
    }

    pub async fn acquire(&mut self) -> Permit {
        struct Acquire
        {
            waiting: bool,
            semaphore: UnsafeCell<*mut Semaphore>,
        }

        impl Future for Acquire
        {
            type Output = Permit;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                unsafe {
                    if (**self.semaphore.get()).permits == 0 {
                        if !self.waiting {
                            (**self.semaphore.get())
                                .waiters
                                .enqueue(cx.waker().clone())
                                .unwrap_or_else(|_| panic!("too many waiters"))
                        }
                        Poll::Pending
                    } else {
                        (**self.semaphore.get()).permits -= 1;
                        Poll::Ready(
                            Permit {
                                address: (**self.semaphore.get()).address.as_ref().unwrap().clone(),
                            }
                        )
                    }
                }
            }
        }

        Acquire {
            waiting: false,
            semaphore: UnsafeCell::new(self),
        }.await
    }

    pub fn release(&mut self) {
        self.permits += 1;
    }
}

impl Actor for Semaphore
{
    fn mount(&mut self, address: Address<Self>) where
        Self: Sized, {
        self.address.replace(address);
    }
}

impl RequestHandler<Acquire>
for Semaphore
{
    type Response = Permit;

    fn on_request(&'static mut self, message: Acquire) -> Response<Self::Response> {
        Response::defer(async move {
            self.acquire().await
        })
    }
}

impl NotifyHandler<Release>
for Semaphore
{
    fn on_notify(&'static mut self, message: Release) -> Completion {
        self.permits += 1;
        if let Some(next) = self.waiters.dequeue() {
            next.wake()
        }
        Completion::immediate()
    }
}

pub struct Permit
{
    address: Address<Semaphore>,
}

impl Drop for Permit
{
    fn drop(&mut self) {
        self.address.notify(Release);
    }
}

impl Address<Semaphore>
{
    pub async fn acquire(&self) -> Permit {
        self.request(Acquire).await
    }
}