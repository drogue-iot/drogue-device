use crate::address::Address;
use core::pin::Pin;
use core::borrow::BorrowMut;
use crate::handler::{AskHandler, TellHandler, Response, Completion};
use core::marker::PhantomData;
use core::future::{Future, Ready};
use core::task::{Context, Poll, Waker};
use core::ptr::replace;

use heapless::{
    Vec,
    consts::*,
};
use crate::supervisor::{Box, alloc_box, alloc};
use core::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

pub trait Actor {
}

pub struct ActorContext<A: Actor> {
    pub(crate) actor: UnsafeCell<A>,
    items: UnsafeCell<Vec<Box<dyn ActorFuture<A>>, U16>>,
}

impl<A: Actor> ActorContext<A> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            items: UnsafeCell::new(Vec::new()),
        }
    }

    fn actor_mut(&'static self) -> &mut A {
        unsafe {
            &mut *self.actor.get()
        }
    }

    pub fn start(&'static self) -> Address<A> {
        drogue_async::task::spawn("actor", self);
        Address::new(self)
    }

    pub(crate) fn tell<M>(&'static self, message: M)
        where A: TellHandler<M>,
              M: 'static
    {
        let tell = alloc(Tell::new(self, message)).unwrap();
        let tell: Box<dyn ActorFuture<A>> = Box::new(tell);
        unsafe {
            //let tell = Pin::new_unchecked(tell);
            (&mut *self.items.get()).push(tell);
        }
    }

    pub(crate) async fn ask<M>(&'static self, message: M) -> <A as AskHandler<M>>::Response
        where A: AskHandler<M>,
              M: 'static
    {
        let signal = alloc(CompletionHandle::new()).unwrap();
        let (sender, receiver) = signal.split();
        let ask = alloc(Ask::new(self, message, sender)).unwrap();
        let response = AskResponseFuture::new(receiver);

        let ask: Box<dyn ActorFuture<A>> = Box::new(ask);

        unsafe {
            //let ask = Pin::new_unchecked(ask);
            (&mut *self.items.get()).push(ask);
        }

        response.await
    }

}

impl<A: Actor> Future for &'static ActorContext<A> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            for item in (&mut *self.items.get()).iter_mut() {
                let result = item.poll(cx);
                match result {
                    Poll::Ready(_) => {}
                    Poll::Pending => {}
                }
            }
        }

        Poll::Pending
    }
}

pub trait ActorFuture<A: Actor> : Future<Output=()>{
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        unsafe {
            Future::poll( Pin::new_unchecked(self), cx)
        }
    }
}

pub struct Tell<A: Actor, M>
    where A: TellHandler<M> + 'static
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
}

impl<A: Actor, M> Tell<A, M>
    where A: TellHandler<M>
{
    pub fn new(actor: &'static ActorContext<A>, message: M) -> Self {
        Self {
            actor,
            message: Some(message),
        }
    }
}

impl<A: Actor + TellHandler<M>, M> ActorFuture<A> for Tell<A, M> {
}

impl<A, M> Unpin for Tell<A, M>
    where A: TellHandler<M>
{}

impl<A: Actor, M> Future for Tell<A, M>
    where A: TellHandler<M>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        //unsafe {
        //let mut handler_context = TellHandlerContext::new(self.actor);
        //self.actor.actor_mut().on_message(self.as_mut().message.take().unwrap(), &mut handler_context);
        //}
        Poll::Ready(())
    }
}

pub struct Ask<A, M>
    where A: Actor + AskHandler<M> + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
    sender: CompletionSender<A::Response>,
    defer: Option<Response<A::Response>>,
}

impl<A, M> Ask<A, M>
    where A: Actor + AskHandler<M> + 'static,
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

impl<A, M> Ask<A, M>
    where A: Actor + AskHandler<M> + 'static,
{}

impl<A: Actor + AskHandler<M>, M> ActorFuture<A> for Ask<A, M> {
}

impl<A, M> Unpin for Ask<A, M>
    where A: Actor + AskHandler<M> + 'static,
{}

impl<A, M> Future for Ask<A, M>
    where A: Actor + AskHandler<M> + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            let mut response = self.actor.actor_mut().on_message(self.as_mut().message.take().unwrap());
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

pub struct AskResponseFuture<R>
    where R: 'static
{
    receiver: CompletionReceiver<R>,
}

impl<R> AskResponseFuture<R> {
    pub fn new(receiver: CompletionReceiver<R>) -> Self {
        Self {
            receiver,
        }
    }
}

impl<R> Future for AskResponseFuture<R> {
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
