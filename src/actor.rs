//! Actor-related types and traits.

use crate::address::Address;
use crate::bus::EventBus;
use crate::handler::{Completion, NotificationHandler, RequestHandler, Response};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use crate::alloc::{alloc, Box, Rc};
use crate::bind::Bind as BindTrait;
use crate::device::Device;
use crate::prelude::Lifecycle;
use crate::supervisor::{actor_executor::ActorState, Supervisor};
use core::cell::{RefCell, UnsafeCell};
// use core::fmt::{Debug, Formatter};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use heapless::spsc::{Consumer, Producer};
use heapless::{consts::*, spsc::Queue};

/// Trait that each actor must implement.
///
/// Each actor must also implement `NotificationHandler<Lifecycle>`.
///
/// See also `NotificationHandler<...>` and `RequestHandler<...>`.
pub trait Actor : NotificationHandler<Lifecycle> {

    /// Called to mount an actor into the system.
    ///
    /// The actor will be present with both its own `Address<...>`
    /// and an instance of the `EventBus<...>` which may be used
    /// to publish messages and events wider.
    ///
    /// The default implementation does nothing.
    fn mount(&mut self, address: Address<Self>)
    where
        Self: Sized,
    {
    }
}

type ItemsProducer<A> = RefCell<Option<Producer<'static, Box<dyn ActorFuture<A>>, U16>>>;
type ItemsConsumer<A> = RefCell<Option<Consumer<'static, Box<dyn ActorFuture<A>>, U16>>>;

/// Struct which is capable of holding an `Actor` instance
/// and connects it to the actor system.
pub struct ActorContext<A: Actor> {
    pub(crate) actor: UnsafeCell<A>,
    pub(crate) current: RefCell<Option<Box<dyn ActorFuture<A>>>>,
    pub(crate) items: UnsafeCell<Queue<Box<dyn ActorFuture<A>>, U16>>,
    pub(crate) items_producer: ItemsProducer<A>,
    pub(crate) items_consumer: ItemsConsumer<A>,
    pub(crate) state_flag_handle: RefCell<Option<*const ()>>,
    pub(crate) in_flight: AtomicBool,
    name: Option<&'static str>,
}

impl<A: Actor> ActorContext<A> {

    /// Create a new context, taking ownership of the provided
    /// actor instance. When mounted, the context and the
    /// contained actor will be moved to the `static` lifetime.
    pub fn new(actor: A) -> Self {
        Self {
            actor: UnsafeCell::new(actor),
            current: RefCell::new(None),
            items: UnsafeCell::new(Queue::new()),
            items_producer: RefCell::new(None),
            items_consumer: RefCell::new(None),
            state_flag_handle: RefCell::new(None),
            in_flight: AtomicBool::new(false),
            name: None,
        }
    }

    /// Provide an optional name for the actor.
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name.replace(name);
        self
    }

    /// Retrieve the name of the actor.
    pub fn name(&self) -> &str {
        self.name.unwrap_or("<unnamed>")
    }

    unsafe fn actor_mut(&'static self) -> &mut A {
        &mut *self.actor.get()
    }

    /// Retrieve an instance of the actor's address.
    pub fn address(&'static self) -> Address<A> {
        Address::new(self)
    }

    pub fn mount(&'static self, supervisor: &mut Supervisor) -> Address<A> {
        let addr = Address::new(self);
        let state_flag_handle = supervisor.activate_actor(self);
        log::trace!("[{}] == {:x}", self.name(), state_flag_handle as u32);
        self.state_flag_handle
            .borrow_mut()
            .replace(state_flag_handle);
        let (producer, consumer) = unsafe { (&mut *self.items.get()).split() };
        self.items_producer.borrow_mut().replace(producer);
        self.items_consumer.borrow_mut().replace(consumer);

        // SAFETY: At this point, we are the only holder of the actor
        unsafe {
            //(&mut *self.state_flag_handle.get()).replace(state_flag_handle);
            (&mut *self.actor.get()).mount(addr.clone() );
        }

        addr
    }

    pub(crate) fn bind<OA: Actor>(&'static self, address: &Address<OA>)
    where
        A: BindTrait<OA>,
        OA: 'static,
    {
        log::trace!("[{}].notify(...)", self.name());
        let bind = alloc(Bind::new(self, address.clone())).unwrap();
        let notify: Box<dyn ActorFuture<A>> = Box::new(bind);
        cortex_m::interrupt::free(|cs| {
            self.items_producer
                .borrow_mut()
                .as_mut()
                .unwrap()
                .enqueue(notify)
                .unwrap_or_else(|_| panic!("too many messages"));
        });

        let flag_ptr = self.state_flag_handle.borrow_mut().unwrap() as *const AtomicU8;
        unsafe {
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }
    }

    pub(crate) fn notify<M>(&'static self, message: M)
    where
        A: NotificationHandler<M>,
        M: 'static,
    {
        log::trace!("[{}].notify(...)", self.name());
        let notify = alloc(Notify::new(self, message)).unwrap();
        let notify: Box<dyn ActorFuture<A>> = Box::new(notify);
        cortex_m::interrupt::free(|cs| {
            self.items_producer
                .borrow_mut()
                .as_mut()
                .unwrap()
                .enqueue(notify)
                .unwrap_or_else(|_| panic!("too many messages"));
        });

        let flag_ptr = self.state_flag_handle.borrow_mut().unwrap() as *const AtomicU8;
        unsafe {
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }
    }

    pub(crate) async fn request<M>(
        &'static self,
        message: M,
    ) -> <A as RequestHandler<M>>::Response
    where
        A: RequestHandler<M>,
        M: 'static,
    {
        // TODO: fix this leak on signals
        //let signal = alloc(CompletionHandle::new()).unwrap();
        let signal = Rc::new(CompletionHandle::new());
        //let (sender, receiver) = signal.split();
        let sender = CompletionSender::new(signal.clone());
        let receiver = CompletionReceiver::new(signal);
        let request = alloc(Request::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new(receiver);

        let request: Box<dyn ActorFuture<A>> = Box::new(request);

        unsafe {
            cortex_m::interrupt::free(|cs| {
                //self.items.borrow_mut().enqueue(request).unwrap_or_else(|_| panic!("too many messages"));
                self.items_producer
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .enqueue(request)
                    .unwrap_or_else(|_| panic!("message queue full"));
            });
            //let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            let flag_ptr = self.state_flag_handle.borrow_mut().unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }

        response.await
    }
}

pub(crate) trait ActorFuture<A: Actor>: Future<Output = ()> + Unpin {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        Future::poll(Pin::new(self), cx)
    }
}

struct Bind<A: Actor, OA: Actor>
where
    A: BindTrait<OA> + 'static,
{
    actor: &'static ActorContext<A>,
    address: Option<Address<OA>>,
}

impl<A: Actor, OA: Actor> Bind<A, OA>
where
    A: BindTrait<OA> + 'static,
{
    fn new(actor: &'static ActorContext<A>, address: Address<OA>) -> Self {
        Self {
            actor,
            address: Some(address),
        }
    }
}

impl<A: Actor + BindTrait<OA>, OA: Actor> ActorFuture<A> for Bind<A, OA> {}

impl<A: Actor + BindTrait<OA>, OA: Actor> Unpin for Bind<A, OA> {}

impl<A: Actor + BindTrait<OA>, OA: Actor> Future for Bind<A, OA> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.address.is_some() {
            log::trace!("[{}] Bind.poll() - dispatch on_bind", self.actor.name());
            unsafe { self.actor.actor_mut() }.on_bind(self.as_mut().address.take().unwrap());
        }
        Poll::Ready(())
    }
}

struct Notify<A: Actor, M>
where
    A: NotificationHandler<M> + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
    defer: Option<Completion>,
}

impl<A: Actor, M> Notify<A, M>
where
    A: NotificationHandler<M>,
{
    pub fn new(actor: &'static ActorContext<A>, message: M) -> Self {
        Self {
            actor,
            message: Some(message),
            defer: None,
        }
    }
}

impl< A: Actor + NotificationHandler<M>, M> ActorFuture<A> for Notify<A, M> {}

impl< A, M> Unpin for Notify<A, M> where A: NotificationHandler<M> + Actor {}

impl<A: Actor, M> Future for Notify<A, M>
where
    A: NotificationHandler<M>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[{}] Notify.poll()", self.actor.name());
        if self.message.is_some() {
            log::trace!(
                "[{}] Notify.poll() - dispatch on_notification",
                self.actor.name()
            );
            let completion = unsafe { self.actor.actor_mut() }
                .on_notification(self.as_mut().message.take().unwrap());
            if matches!(completion, Completion::Immediate()) {
                log::trace!("[{}] Notify.poll() - immediate: Ready", self.actor.name());
                return Poll::Ready(());
            }
            self.defer.replace(completion);
        }

        log::trace!("[{}] Notify.poll() - check defer", self.actor.name());
        if let Some(Completion::Defer(ref mut fut)) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(response) => {
                    log::trace!("[{}] Notify.poll() - defer: Ready", self.actor.name());
                    //self.sender.send(response);
                    self.defer.take();
                    Poll::Ready(())
                }
                Poll::Pending => {
                    log::trace!("[{}] Notify.poll() - defer: Pending", self.actor.name());
                    Poll::Pending
                }
            }
        } else {
            log::trace!("[{}] Notify.poll() - ERROR - no defer?", self.actor.name());
            // should not actually get here ever
            Poll::Ready(())
        }
    }
}

struct Request<A, M>
where
    A: Actor + RequestHandler<M> + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<M>,
    sender: CompletionSender<A::Response>,
    defer: Option<Response<A::Response>>,
}

impl<A, M> Request<A, M>
where
    A: Actor + RequestHandler<M> + 'static,
{
    pub fn new(
        actor: &'static ActorContext<A>,
        message: M,
        sender: CompletionSender<A::Response>,
    ) -> Self {
        Self {
            actor,
            message: Some(message),
            sender,
            defer: None,
        }
    }
}

impl<A, M> Request<A, M> where A: Actor + RequestHandler<M> + 'static {}

impl<A: Actor + RequestHandler<M>, M> ActorFuture<A> for Request<A, M> {}

impl<A, M> Unpin for Request<A, M> where A: Actor + RequestHandler<M> + 'static {}

impl<A, M> Future for Request<A, M>
where
    A: Actor + RequestHandler<M> + 'static,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[{}] Request.poll()", self.actor.name());
        if self.message.is_some() {
            let response =
                unsafe { self.actor.actor_mut() }.on_request(self.as_mut().message.take().unwrap());

            match response {
                Response::Immediate(val) => {
                    self.sender.send_value(val);
                    return Poll::Ready(());
                }
                Response::ImmediateFuture(fut) => {
                    self.sender.send_future(fut);
                    return Poll::Ready(());
                }
                defer @ Response::Defer(_) => {
                    self.defer.replace(defer);
                }
            }
            /*
            if let Response::Immediate(response) = response {
                self.sender.send(response);
                return Poll::Ready(());
            } else {
                self.defer.replace(response);
            }
             */
        }

        if let Some(Response::Defer(ref mut fut)) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(response) => {
                    self.sender.send_value(response);
                    self.defer.take();
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            // should not actually get here ever
            Poll::Ready(())
        }
    }
}

struct RequestResponseFuture<R>
where
    R: 'static,
{
    receiver: CompletionReceiver<R>,
}

impl<R> RequestResponseFuture<R> {
    pub fn new(receiver: CompletionReceiver<R>) -> Self {
        Self { receiver }
    }
}

impl<R> Future for RequestResponseFuture<R> {
    type Output = R;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll(cx)
    }
}

struct CompletionHandle<T> {
    value: RefCell<Option<CompletionValue<T>>>,
    waker: RefCell<Option<Waker>>,
}

enum CompletionValue<T> {
    Immediate(T),
    Future(Box<dyn Future<Output = T>>),
}

impl<T> CompletionHandle<T> {
    pub fn new() -> Self {
        Self {
            value: RefCell::new(None),
            waker: RefCell::new(None),
        }
    }
}

impl<T> Default for CompletionHandle<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> CompletionHandle<T> {
    pub fn send_value(&self, value: T) {
        self.value
            .borrow_mut()
            .replace(CompletionValue::Immediate(value));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake()
        }
    }

    pub fn send_future(&self, value: Box<dyn Future<Output = T>>) {
        self.value
            .borrow_mut()
            .replace(CompletionValue::Future(value));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake()
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<T> {
        if self.value.borrow().is_none() {
            self.waker.borrow_mut().replace(cx.waker().clone());
            Poll::Pending
        } else {
            let mut v = self.value.borrow_mut().take().unwrap();
            match v {
                CompletionValue::Immediate(val) => Poll::Ready(val),
                CompletionValue::Future(ref mut fut) => {
                    let fut = Pin::new(fut);
                    let result = fut.poll(cx);
                    if let Poll::Pending = result {
                        self.value.borrow_mut().replace(v);
                    }
                    result
                }
            }
            //Poll::Ready(self.value.borrow_mut().take().unwrap())
        }
    }
}

struct CompletionSender<T: 'static> {
    handle: Rc<CompletionHandle<T>>,
}

impl<T: 'static> CompletionSender<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>>) -> Self {
        Self { handle }
    }

    pub(crate) fn send_value(&self, response: T) {
        self.handle.send_value(response);
    }

    pub(crate) fn send_future(&self, response: Box<dyn Future<Output = T>>) {
        self.handle.send_future(response);
    }
}

struct CompletionReceiver<T: 'static> {
    handle: Rc<CompletionHandle<T>>,
}

impl<T: 'static> CompletionReceiver<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>>) -> Self {
        Self { handle }
    }

    pub(crate) fn poll(&self, cx: &mut Context) -> Poll<T> {
        self.handle.poll(cx)
    }
}
