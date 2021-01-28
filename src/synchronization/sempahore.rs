use crate::prelude::{Actor, Device, NotificationHandler, Lifecycle, Address};
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

pub struct Semaphore<D>
    where
        D: Device
{
    address: Option<Address<D, Self>>,
    permits: usize,
    waiters: Queue<Waker, U16>,
}

impl<D> Semaphore<D>
    where
        D: Device + 'static,
{
    pub fn new(permits: usize) -> Self {
        Self {
            address: None,
            permits,
            waiters: Queue::new(),
        }
    }

    pub async fn acquire(&mut self) -> Permit<D> {
        struct Acquire<D>
            where D: Device
        {
            waiting: bool,
            semaphore: UnsafeCell<*mut Semaphore<D>>,
        }

        impl<D> Future for Acquire<D>
            where
                D: Device + 'static
        {
            type Output = Permit<D>;

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

impl<D> Actor<D> for Semaphore<D>
    where
        D: Device,
{
    fn mount(&mut self, address: Address<D, Self>, bus: EventBus<D>) where
        Self: Sized, {
        self.address.replace(address);
    }
}

impl<D> NotificationHandler<Lifecycle> for Semaphore<D>
    where
        D: Device,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<D> RequestHandler<D, Acquire>
for Semaphore<D>
    where
        D: Device + 'static,
{
    type Response = Permit<D>;

    fn on_request(&'static mut self, message: Acquire) -> Response<Self::Response> {
        Response::defer( async move {
            self.acquire().await
        } )
    }
}

impl<D> NotificationHandler<Release>
for Semaphore<D>
    where
        D: Device,
{
    fn on_notification(&'static mut self, message: Release) -> Completion {
        self.permits += 1;
        if let Some(next) = self.waiters.dequeue() {
            next.wake()
        }
        Completion::immediate()
    }
}

pub struct Permit<D>
    where
        D: Device + 'static,
{
    address: Address<D, Semaphore<D>>,
}

impl<D> Drop for Permit<D>
    where
        D: Device + 'static,
{
    fn drop(&mut self) {
        self.address.notify( Release );
    }
}

impl<D> Address<D, Semaphore<D>>
    where
        D: Device + 'static,
{

    pub async fn acquire(&self) -> Permit<D> {
        self.request( Acquire ).await
    }

}