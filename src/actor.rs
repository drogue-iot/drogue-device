use crate::address::Address;
use core::pin::Pin;
use crate::handler::{RequestHandler, NotificationHandler, Response, Completion};
use core::future::Future;
use core::task::{Context, Poll, Waker};

use heapless::{
    spsc::Queue,
    consts::*,
};
use crate::alloc::{Box, Rc, alloc};
use core::cell::UnsafeCell;
use crate::supervisor::{
    Supervisor,
    actor_executor::ActorState,
};
use core::sync::atomic::{AtomicU8, Ordering, AtomicBool};
use crate::interrupt::Interrupt;
use cortex_m::peripheral::NVIC;
use cortex_m::interrupt::Nr;
use core::fmt::{Debug, Formatter};


pub trait Actor {
    fn start(&mut self, address: Address<Self>)
        where Self: Sized
    {}
}

pub struct ActorContext<A: Actor> {
    pub(crate) actor: UnsafeCell<A>,
    pub(crate) current: UnsafeCell<Option<Box<dyn ActorFuture<A>>>>,
    pub(crate) items: UnsafeCell<Queue<Box<dyn ActorFuture<A>>, U16>>,
    pub(crate) state_flag_handle: UnsafeCell<Option<*const ()>>,
    pub(crate) irq: Option<u8>,
    pub(crate) in_flight: AtomicBool,
}

impl<A: Actor> ActorContext<A> {
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            current: UnsafeCell::new(None),
            items: UnsafeCell::new(Queue::new()),
            state_flag_handle: UnsafeCell::new(None),
            irq: None,
            in_flight: AtomicBool::new(false),
        }
    }

    pub(crate) fn new_interrupt(actor: A, irq: u8) -> Self
        where A: Interrupt
    {
        Self {
            actor: UnsafeCell::new(actor),
            current: UnsafeCell::new(None),
            items: UnsafeCell::new(Queue::new()),
            state_flag_handle: UnsafeCell::new(None),
            irq: Some(irq),
            in_flight: AtomicBool::new(false),
        }
    }

    fn actor_mut(&'static self) -> &mut A {
        unsafe {
            &mut *self.actor.get()
        }
    }


    pub fn start(&'static self, supervisor: &mut Supervisor) -> Address<A> {
        let addr = Address::new(self);
        let state_flag_handle = supervisor.activate_actor(self);
        unsafe {
            (&mut *self.state_flag_handle.get()).replace(state_flag_handle);
            (&mut *self.actor.get()).start(addr.clone());
        }

        addr
    }

    pub(crate) fn start_interrupt(&'static self, supervisor: &mut Supervisor) -> Address<A>
        where A: Interrupt
    {
        let addr = self.start(supervisor);
        supervisor.activate_interrupt(self, self.irq.unwrap());

        if let Some(irq) = self.irq {
            struct IrqNr(u8);
            unsafe impl Nr for IrqNr {
                fn nr(&self) -> u8 {
                    self.0
                }
            }
            unsafe {
                NVIC::unmask(IrqNr(irq))
            }
        }

        addr
    }

    pub(crate) fn notify<M>(&'static self, message: M)
        where A: NotificationHandler<M>,
              M: 'static
    {
        let notify = alloc(Notify::new(self, message)).unwrap();
        let notify: Box<dyn ActorFuture<A>> = Box::new(notify);
        unsafe {
            cortex_m::interrupt::free(|cs| {
                (&mut *self.items.get()).enqueue(notify).unwrap_or_else(|_| panic!("too many messages"));
            });
            let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }
    }

    pub(crate) async fn request<M>(&'static self, message: M) -> <A as RequestHandler<M>>::Response
        where A: RequestHandler<M>,
              M: 'static
    {
        // TODO: fix this leak on signals
        //let signal = alloc(CompletionHandle::new()).unwrap();
        let signal = Rc::new( CompletionHandle::new() );
        //let (sender, receiver) = signal.split();
        let sender = CompletionSender::new(signal.clone());
        let receiver = CompletionReceiver::new(signal);

        let request = alloc(Request::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new(receiver);

        let request: Box<dyn ActorFuture<A>> = Box::new(request);

        unsafe {
            cortex_m::interrupt::free(|cs| {
                (&mut *self.items.get()).enqueue(request).unwrap_or_else(|_| panic!("too many messages"));
            });
            let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }

        log::info!("WAIT response");
        let r = response.await;
        log::info!("WAIT response FINISH");
        r
    }
}

pub(crate) trait ActorFuture<A: Actor>: Future<Output=()> + Debug {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        unsafe {
            Future::poll(Pin::new_unchecked(self), cx)
        }
    }
}

struct Notify<A: Actor, M>
    where A: NotificationHandler<M> + 'static
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
    defer: Option<Completion>,
}

impl<A: Actor, M> Notify<A, M>
    where A: NotificationHandler<M>
{
    pub fn new(actor: &'static ActorContext<A>, message: M) -> Self {
        Self {
            actor,
            message: Some(message),
            defer: None,
        }
    }
}

impl<A: Actor + NotificationHandler<M>, M> ActorFuture<A> for Notify<A, M> {}

impl<A: Actor + NotificationHandler<M>, M> Debug for Notify<A, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("Notify")
    }
}


impl<A, M> Unpin for Notify<A, M>
    where A: NotificationHandler<M>
{}

impl<A: Actor, M> Future for Notify<A, M>
    where A: NotificationHandler<M>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.message.is_some() {
            let mut completion = self.actor.actor_mut().on_notification(self.as_mut().message.take().unwrap());
            if matches!( completion, Completion::Immediate() ) {
                return Poll::Ready(());
            }
            self.defer.replace(completion);
        }

        if let Some(Completion::Defer(ref mut fut)) = &mut self.defer {
            unsafe {
                let fut = Pin::new_unchecked(fut);
                let result = fut.poll(cx);
                match result {
                    Poll::Ready(response) => {
                        //self.sender.send(response);
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

struct Request<A, M>
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

impl<A: Actor + RequestHandler<M>, M> ActorFuture<A> for Request<A, M> {}

impl<A: Actor + RequestHandler<M>, M> Debug for Request<A, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("Request")
    }
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

struct RequestResponseFuture<R>
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
        log::info!("about to poll response receiver");
        let result = self.receiver.poll(cx);
        if result.is_ready() {
            log::info!("result is ready");
        } else {
            log::info!("result is pending");
        }
        result
    }
}

struct CompletionHandle<T> {
    value: UnsafeCell<Option<T>>,
    waker: UnsafeCell<Option<Waker>>,
}

impl<T> Drop for CompletionHandle<T> {
    fn drop(&mut self) {
        log::info!("dropping CompletionHandle");
    }
}

impl<T> CompletionHandle<T> {
    pub fn new() -> Self {
        Self {
            value: UnsafeCell::new(None),
            waker: UnsafeCell::new(None),
        }
    }
}

impl<T> Default for CompletionHandle<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> CompletionHandle<T> {
    pub fn send(&self, value: T) {
        unsafe {
            (&mut *self.value.get()).replace(value);
            log::info!("sending a response");
            if let Some(waker) = (&mut *self.waker.get()).take() {
                log::info!("waking a waker");
                waker.wake()
            }
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<T> {
        unsafe {
            log::info!("polling response");
            if (&*self.value.get()).is_none() {
                log::info!("registering waker and pending");
                (&mut *self.waker.get()).replace(cx.waker().clone());
                Poll::Pending
            } else {
                log::info!("saying is ready");
                Poll::Ready((&mut *self.value.get()).take().unwrap())
            }
        }
    }
}

struct CompletionSender<T: 'static> {
    handle: Rc<CompletionHandle<T>>,
}

impl<T: 'static> CompletionSender<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>>) -> Self {
        Self {
            handle
        }
    }

    pub(crate) fn send(&self, response: T) {
        self.handle.send(response);
    }
}

struct CompletionReceiver<T: 'static> {
    handle: Rc<CompletionHandle<T>>,
}

impl<T: 'static> CompletionReceiver<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>>) -> Self {
        Self {
            handle
        }
    }

    pub(crate) fn poll(&self, cx: &mut Context) -> Poll<T> {
        log::info!("polling a response receiver");
        self.handle.poll(cx)
    }
}
