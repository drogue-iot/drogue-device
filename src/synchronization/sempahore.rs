//! A semaphore actor and supporting types.

use crate::prelude::*;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use heapless::{consts::*, spsc::Queue};

#[derive(Debug)]
pub struct Acquire;
pub struct Release;

pub struct Shared {
    permits: RefCell<usize>,
    waiters: RefCell<Queue<Waker, U16>>,
}

impl Shared {
    fn new(permits: usize) -> Self {
        Self {
            permits: RefCell::new(permits),
            waiters: RefCell::new(Queue::new()),
        }
    }

    fn acquire(&self) -> bool {
        let permits = *self.permits.borrow();
        if permits > 0 {
            *self.permits.borrow_mut() = permits - 1;
            true
        } else {
            false
        }
    }

    fn release(&self) {
        let permits = *self.permits.borrow();
        *self.permits.borrow_mut() = permits + 1;
        if let Some(next) = self.waiters.borrow_mut().dequeue() {
            next.wake()
        }
    }

    fn waiting(&self, waker: &Waker) {
        self.waiters.borrow_mut().enqueue(waker.clone()).ok();
    }
}

pub struct Semaphore {
    shared: Shared,
    actor: ActorContext<SemaphoreActor>,
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            shared: Shared::new(permits),
            actor: ActorContext::new(SemaphoreActor::new()),
        }
    }
}

impl Package for Semaphore {

    type Primary = SemaphoreActor;
    type Configuration = ();

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        self.actor.mount(&self.shared, supervisor)
    }
}

pub struct SemaphoreActor {
    address: Option<Address<Self>>,
    shared: Option<&'static Shared>,
}

impl Configurable for SemaphoreActor {
    type Configuration = &'static Shared;

    fn configure(&mut self, config: Self::Configuration) {
        self.shared.replace(config);
    }
}

impl Default for SemaphoreActor {
    fn default() -> Self {
        Self::new()
    }
}

impl SemaphoreActor {
    fn new() -> Self {
        Self {
            address: None,
            shared: None,
        }
    }

    pub async fn acquire(&self) -> Permit {
        struct Acquire {
            waiting: bool,
            address: Address<SemaphoreActor>,
            shared: &'static Shared,
        }

        impl Future for Acquire {
            type Output = Permit;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.shared.acquire() {
                    return Poll::Ready(Permit {
                        address: self.address,
                    });
                }
                if !self.waiting {
                    self.shared.waiting(cx.waker());
                    self.waiting = true;
                }

                Poll::Pending
            }
        }

        Acquire {
            waiting: false,
            address: self.address.unwrap(),
            shared: self.shared.unwrap(),
        }
        .await
    }

    pub fn release(&self) {
        self.shared.unwrap().release();
    }
}

impl Actor for SemaphoreActor {
    type Configuration = &'static Shared;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.address.replace(address);
        self.shared.replace( config );
    }
}

impl RequestHandler<Acquire> for SemaphoreActor {
    type Response = Permit;

    fn on_request(self, message: Acquire) -> Response<Self, Self::Response> {
        Response::defer(async move {
            let sempaphore = self.acquire().await;
            self.respond_with(sempaphore)
        })
    }
}

impl NotifyHandler<Release> for SemaphoreActor {
    fn on_notify(self, message: Release) -> Completion<Self> {
        self.release();
        Completion::immediate(self)
    }
}

pub struct Permit {
    address: Address<SemaphoreActor>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.address.notify(Release);
    }
}

impl Address<SemaphoreActor> {
    pub async fn acquire(&self) -> Permit {
        self.request(Acquire).await
    }
}
