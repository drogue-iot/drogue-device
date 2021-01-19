use crate::address::Address;
use core::pin::Pin;
use crate::handler::{RequestHandler, NotificationHandler, Response, Completion};
use core::future::Future;
use core::task::{Context, Poll, Waker};

use heapless::{
    Vec,
    spsc::Queue,
    consts::*,
};
use crate::alloc::{Box, alloc};
use core::cell::UnsafeCell;
use crate::supervisor::{Supervisor, ActorState};
use core::sync::atomic::{AtomicU8, Ordering};


pub trait Actor {
}

pub struct ActorContext<A: Actor> {
    pub(crate) actor: UnsafeCell<A>,
    pub(crate) current: UnsafeCell<Option<Box<dyn ActorFuture<A>>>>,
    pub(crate) items: UnsafeCell<Queue<Box<dyn ActorFuture<A>>, U16>>,
    pub(crate) state_flag_handle: UnsafeCell<Option<* const ()>>,
}

impl<A: Actor> ActorContext<A> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            current: UnsafeCell::new(None),
            items: UnsafeCell::new(Queue::new()),
            state_flag_handle: UnsafeCell::new(None),
        }
    }

    fn actor_mut(&'static self) -> &mut A {
        unsafe {
            &mut *self.actor.get()
        }
    }


    pub fn start(&'static self, supervisor: &mut Supervisor) -> Address<A> {
        supervisor.activate_actor( self );
        Address::new(self)
    }

    pub(crate) fn notify<M>(&'static self, message: M)
        where A: NotificationHandler<M>,
              M: 'static
    {
        let notify = alloc(Notify::new(self, message)).unwrap();
        let notify: Box<dyn ActorFuture<A>> = Box::new(notify);
        unsafe {
            (&mut *self.items.get()).enqueue(notify);
            let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }
    }

    pub(crate) async fn request<M>(&'static self, message: M) -> <A as RequestHandler<M>>::Response
        where A: RequestHandler<M>,
              M: 'static
    {
        let signal = alloc(CompletionHandle::new()).unwrap();
        let (sender, receiver) = signal.split();
        let request = alloc(Request::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new(receiver);

        let request: Box<dyn ActorFuture<A>> = Box::new(request);

        unsafe {
            (&mut *self.items.get()).enqueue(request);
            let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }

        response.await
    }

}

pub trait ActorFuture<A: Actor> : Future<Output=()>{
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        unsafe {
            Future::poll( Pin::new_unchecked(self), cx)
        }
    }
}

pub struct Notify<A: Actor, M>
    where A: NotificationHandler<M> + 'static
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
}

impl<A: Actor, M> Notify<A, M>
    where A: NotificationHandler<M>
{
    pub fn new(actor: &'static ActorContext<A>, message: M) -> Self {
        Self {
            actor,
            message: Some(message),
        }
    }
}

impl<A: Actor + NotificationHandler<M>, M> ActorFuture<A> for Notify<A, M> {
}

impl<A, M> Unpin for Notify<A, M>
    where A: NotificationHandler<M>
{
}

impl<A: Actor, M> Future for Notify<A, M>
    where A: NotificationHandler<M>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            let mut result = self.actor.actor_mut().on_notification(self.as_mut().message.take().unwrap() );
            match result {
                Completion::Immediate() => {
                    Poll::Ready(())
                }
                Completion::Defer(ref mut f) => {
                    unsafe {
                        Pin::new_unchecked(f).poll(cx)
                    }
                }
            }
        } else {
            Poll::Ready(())
        }
    }
}

pub struct Request<A, M>
    where A: Actor + RequestHandler<M> + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
    sender: CompletionSender<A::Response>,
    defer: Option<Response<A::Response>>,
}

impl<A, M> Request<A, M>
    where A: Actor + RequestHandler<M> + 'static,
{
    pub fn new(actor: &'static ActorContext<A>, message: M, sender: CompletionSender<A::Response>) -> Self {
        Self {
            actor,
            message: Some(message),
            sender,
            defer: None,
        }
    }
}

impl<A, M> Request<A, M>
    where A: Actor + RequestHandler<M> + 'static,
{}

impl<A: Actor + RequestHandler<M>, M> ActorFuture<A> for Request<A, M> {
}

impl<A, M> Unpin for Request<A, M>
    where A: Actor + RequestHandler<M> + 'static,
{}

impl<A, M> Future for Request<A, M>
    where A: Actor + RequestHandler<M> + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            let response = self.actor.actor_mut().on_request(self.as_mut().message.take().unwrap());
            if let Response::Immediate(response) = response {
                self.sender.send(response);
                return Poll::Ready(());
            } else {
                self.defer.replace(response);
            }
        }

        if let Some(Response::Defer(ref mut fut)) = &mut self.defer {
            unsafe {
                let fut = Pin::new_unchecked(fut);
                let result = fut.poll(cx);
                match result {
                    Poll::Ready(response) => {
                        self.sender.send(response);
                        Poll::Ready(())
                    }
                    Poll::Pending => {
                        Poll::Pending
                    }
                }
            }
        } else {
            // should not actually get here ever
            Poll::Ready(())
        }
    }
}

pub struct RequestResponseFuture<R>
    where R: 'static
{
    receiver: CompletionReceiver<R>,
}

impl<R> RequestResponseFuture<R> {
    pub fn new(receiver: CompletionReceiver<R>) -> Self {
        Self {
            receiver,
        }
    }
}

impl<R> Future for RequestResponseFuture<R> {
    type Output = R;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll(cx)
    }
}

pub struct CompletionHandle<T> {
    value: UnsafeCell<Option<T>>,
    waker: UnsafeCell<Option<Waker>>,
}

impl<T> CompletionHandle<T> {
    pub fn new() -> Self {
        Self {
            value: UnsafeCell::new(None),
            waker: UnsafeCell::new(None),
        }
    }

    pub fn split(&'static self) -> (CompletionSender<T>, CompletionReceiver<T>) {
        (
            CompletionSender::new(self),
            CompletionReceiver::new(self),
        )
    }
}

impl<T> CompletionHandle<T> {
    pub fn send(&'static self, value: T) {
        unsafe {
            (&mut *self.value.get()).replace(value);
            if let Some(waker) = (&mut *self.waker.get()).take() {
                waker.wake()
            }
        }
    }

    pub fn poll(&'static self, cx: &mut Context<'_>) -> Poll<T> {
        unsafe {
            if (&*self.value.get()).is_none() {
                (&mut *self.waker.get()).replace(cx.waker().clone());
                Poll::Pending
            } else {
                Poll::Ready((&mut *self.value.get()).take().unwrap())
            }
        }
    }
}

pub struct CompletionSender<T: 'static> {
    handle: &'static CompletionHandle<T>,
}

impl<T: 'static> CompletionSender<T> {
    pub(crate) fn new(handle: &'static CompletionHandle<T>) -> Self {
        Self {
            handle
        }
    }

    pub(crate) fn send(&self, response: T) {
        self.handle.send(response);
    }
}

pub struct CompletionReceiver<T: 'static> {
    handle: &'static CompletionHandle<T>,
}

impl<T: 'static> CompletionReceiver<T> {
    pub(crate) fn new(handle: &'static CompletionHandle<T>) -> Self {
        Self {
            handle
        }
    }

    pub(crate) fn poll(&self, cx: &mut Context) -> Poll<T> {
        self.handle.poll(cx)
    }
}
