//! Actor-related types and traits.

use crate::arena::Arena;
use core::cell::{RefCell, UnsafeCell};
use core::future::Future;
use core::mem::transmute;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use core::task::{Context, Poll, Waker};

use heapless::spsc::{Consumer, Producer};
use heapless::{consts::*, spsc::Queue, String};

use crate::arch::with_critical_section;
use crate::arena::{Box, Rc};
use crate::prelude::*;
use crate::system::device::Lifecycle;
use crate::system::supervisor::actor_executor::ActiveActor;
use crate::system::supervisor::{actor_executor::ActorState, Supervisor};
use crate::system::SystemArena;

pub trait Configurable {
    type Configuration;
    fn configure(&mut self, config: Self::Configuration);
}

/// Trait that each actor must implement.
pub trait Actor: Sized {
    type Configuration;
    type Request;
    type Response;

    /// Called to mount an actor into the system.
    ///
    /// The actor will be presented with both its own `Address<...>`.
    ///
    /// The default implementation does nothing.
    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
    }

    /// Lifecycle event of *initialize*.
    fn on_initialize(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }

    /// Lifecycle event of *start*.
    fn on_start(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }

    /// Lifecycle event of *sleep*. *Unused currently*.
    fn on_sleep(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }

    /// Lifecycle event of *hibernate*. *Unused currently*.
    fn on_hibernate(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }

    /// Lifecycle event of *stop*. *Unused currently*.
    fn on_stop(self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }

    fn on_request(self, request: Self::Request) -> Response<Self>;
}

/// Global methods for acquiring the current actor's infomation.
pub struct ActorInfo {
    pub(crate) name: Option<&'static str>,
}

impl ActorInfo {
    /// Retrieve the current actor's name, if set, else probably just `"<unnamed>"`
    pub fn name() -> &'static str {
        unsafe { CURRENT.name.unwrap_or("<unnamed>") }
    }
}

pub(crate) static mut CURRENT: ActorInfo = ActorInfo { name: None };

type ItemsProducer<A> =
    RefCell<Option<Producer<'static, Box<dyn ActorFuture<A>, SystemArena>, U8>>>;
type ItemsConsumer<A> =
    RefCell<Option<Consumer<'static, Box<dyn ActorFuture<A>, SystemArena>, U8>>>;

/// Struct which is capable of holding an `Actor` instance
/// and connects it to the actor system.
pub struct ActorContext<A: Actor + 'static> {
    pub(crate) actor: RefCell<Option<A>>,
    pub(crate) current: RefCell<Option<Box<dyn ActorFuture<A>, SystemArena>>>,
    // Only an UnsafeCell instead of RefCell in order to maintain it's 'static nature when borrowed.
    pub(crate) items: UnsafeCell<Queue<Box<dyn ActorFuture<A>, SystemArena>, U8>>,
    pub(crate) items_producer: ItemsProducer<A>,
    pub(crate) items_consumer: ItemsConsumer<A>,
    //pub(crate) items: FutureQueue<A>,
    pub(crate) state_flag_handle: RefCell<Option<*const ()>>,
    pub(crate) in_flight: AtomicBool,
    name: Option<&'static str>,
}

impl<A: Actor + 'static> ActorContext<A> {
    /// Create a new context, taking ownership of the provided
    /// actor instance. When mounted, the context and the
    /// contained actor will be moved to the `static` lifetime.
    pub fn new(actor: A) -> Self {
        Self {
            actor: RefCell::new(Some(actor)),
            current: RefCell::new(None),
            //items: FutureQueue::new(),
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
    pub fn name(&self) -> &'static str {
        self.name.unwrap_or("<unnamed>")
    }

    #[inline(always)]
    fn take_actor(&self) -> Option<A> {
        self.actor.borrow_mut().take()
    }

    #[inline(always)]
    fn replace_actor(&self, actor: A) {
        self.actor.borrow_mut().replace(actor);
    }

    /// Retrieve an instance of the actor's address.
    pub fn address(&'static self) -> Address<A> {
        Address::new(self)
    }

    /// Mount the context and its actor into the system.
    pub fn mount(
        &'static self,
        config: A::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<A> {
        let addr = Address::new(self);
        let (actor_index, state_flag_handle) = supervisor.activate_actor(self);
        log::trace!("[{}] == {:x}", self.name(), state_flag_handle as u32);
        self.state_flag_handle
            .borrow_mut()
            .replace(state_flag_handle);
        let (producer, consumer) = unsafe { (&mut *self.items.get()).split() };
        self.items_producer.borrow_mut().replace(producer);
        self.items_consumer.borrow_mut().replace(consumer);

        self.actor
            .borrow_mut()
            .as_mut()
            .unwrap()
            .on_mount(addr, config);

        addr
    }

    /// Dispatch a lifecycle event.
    pub(crate) fn lifecycle(&'static self, event: Lifecycle) {
        log::trace!("[{}].lifecycle({:?})", self.name(), event);

        let lifecycle = SystemArena::alloc(OnLifecycle::new(self, event)).unwrap();
        let lifecycle: Box<dyn ActorFuture<A>, SystemArena> = Box::new(lifecycle);

        with_critical_section(|cs| {
            self.items_producer
                .borrow_mut()
                .as_mut()
                .unwrap()
                .enqueue(lifecycle)
                .unwrap_or_else(|_| panic!("too many messages"));
        });

        let flag_ptr = self.state_flag_handle.borrow_mut().unwrap() as *const AtomicU8;
        unsafe {
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }

        let _ = self.do_poll(self.state_flag_handle.borrow().unwrap());
    }

    /// Dispatch a notification.
    pub(crate) fn notify(&'static self, message: A::Request) {
        log::trace!("[{}].notify(...)", self.name());

        let request: &mut dyn ActorFuture<A> =
            SystemArena::alloc(OnNotify::new(self, message)).unwrap();

        unsafe {
            let request: Box<dyn ActorFuture<A>, SystemArena> = Box::new(request);
            with_critical_section(|cs| {
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
    }

    /// Dispatch an async request.
    pub(crate) async fn request(&'static self, message: A::Request) -> A::Response {
        let signal = Rc::new(CompletionHandle::new());
        let sender = CompletionSender::new(signal.clone());
        let receiver = CompletionReceiver::new(signal);
        let request: &mut dyn ActorFuture<A> =
            SystemArena::alloc(OnRequest::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new(receiver);

        unsafe {
            let request: Box<dyn ActorFuture<A>, SystemArena> = Box::new(request);
            with_critical_section(|cs| {
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

    /// Dispatch an async request.
    pub(crate) async fn request_cancellable(&'static self, message: A::Request) -> A::Response {
        let signal = Rc::new(CompletionHandle::new());
        let sender = CompletionSender::new(signal.clone());
        let receiver = CompletionReceiver::new(signal);
        let request: &mut dyn ActorFuture<A> =
            SystemArena::alloc(OnRequest::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new(receiver);

        unsafe {
            let request = transmute::<_, &mut (dyn ActorFuture<A> + 'static)>(request);
            let request: Box<dyn ActorFuture<A>, SystemArena> = Box::new(request);
            with_critical_section(|cs| {
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

    /// Dispatch an async request.
    pub(crate) async fn request_panicking(&'static self, message: A::Request) -> A::Response {
        let signal = Rc::new(CompletionHandle::new());
        let sender = CompletionSender::new(signal.clone());
        let receiver = CompletionReceiver::new(signal);

        let debug = "unknown".into();
        //write!(debug, "{:?}", type_name::<M>()).unwrap();

        let request: &mut dyn ActorFuture<A> =
            SystemArena::alloc(OnRequest::new(self, message, sender)).unwrap();
        let response = RequestResponseFuture::new_panicking(receiver, debug);

        unsafe {
            let request = transmute::<_, &mut (dyn ActorFuture<A> + 'static)>(request);
            let request: Box<dyn ActorFuture<A>, SystemArena> = Box::new(request);
            with_critical_section(|cs| {
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

    pub(crate) fn interrupt(&self)
    where
        A: Interrupt,
    {
        self.actor.borrow_mut().as_mut().unwrap().on_interrupt()
    }
}

pub(crate) trait ActorFuture<A: Actor>: Future<Output = ()> + Unpin {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        Future::poll(Pin::new(self), cx)
    }
}

struct OnLifecycle<A: Actor + 'static> {
    actor: &'static ActorContext<A>,
    event: Lifecycle,
    defer: Option<Completion<A>>,
    dispatched: bool,
}

impl<A: Actor> OnLifecycle<A> {
    fn new(actor: &'static ActorContext<A>, event: Lifecycle) -> Self {
        Self {
            actor,
            event,
            defer: None,
            dispatched: false,
        }
    }
}

impl<A: Actor> ActorFuture<A> for OnLifecycle<A> {}

impl<A: Actor> Unpin for OnLifecycle<A> {}

impl<A: Actor> Future for OnLifecycle<A> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[{}] Lifecycle.poll() {:?}", self.actor.name(), self.event);
        if !self.dispatched {
            let actor = self.actor.take_actor().expect("actor is missing");
            log::trace!(
                "[{}] Lifecycle.poll() - dispatch on_lifecycle {:?}",
                self.actor.name(),
                self.event
            );
            let completion = match self.event {
                Lifecycle::Initialize => actor.on_initialize(),
                Lifecycle::Start => actor.on_start(),
                Lifecycle::Stop => actor.on_stop(),
                Lifecycle::Sleep => actor.on_sleep(),
                Lifecycle::Hibernate => actor.on_hibernate(),
            };
            self.dispatched = true;
            match completion {
                Completion::Immediate(actor) => {
                    self.actor.replace_actor(actor);
                    log::trace!(
                        "[{}] Lifecycle.poll() - immediate: Ready",
                        self.actor.name()
                    );
                    return Poll::Ready(());
                }
                Completion::Defer(_) => {
                    self.defer.replace(completion);
                }
            }
        }

        log::trace!(
            "[{}] Lifecycle.poll() - check defer {:?}",
            self.actor.name(),
            self.event
        );
        if let Some(Completion::Defer(ref mut fut)) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(actor) => {
                    log::trace!(
                        "[{}] Lifecycle.poll() - defer: Ready {:?}",
                        self.actor.name(),
                        self.event
                    );
                    self.actor.replace_actor(actor);
                    //self.sender.send(response);
                    self.defer.take();
                    Poll::Ready(())
                }
                Poll::Pending => {
                    log::trace!(
                        "[{}] Lifecycle.poll() - defer: Pending {:?}",
                        self.actor.name(),
                        self.event
                    );
                    Poll::Pending
                }
            }
        } else {
            log::trace!(
                "[{}] Lifecycle.poll() - ERROR - no defer?",
                self.actor.name()
            );
            // should not actually get here ever
            Poll::Ready(())
        }
    }
}

struct OnNotify<A>
where
    A: Actor + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<A::Request>,
    defer: Option<Response<A>>,
}

impl<A> OnNotify<A>
where
    A: Actor,
{
    pub fn new(actor: &'static ActorContext<A>, message: A::Request) -> Self {
        Self {
            actor,
            message: Some(message),
            defer: None,
        }
    }
}

impl<A> ActorFuture<A> for OnNotify<A> where A: Actor {}

impl<A> Unpin for OnNotify<A> where A: Actor {}

impl<A> Future for OnNotify<A>
where
    A: Actor,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[{}] Notify.poll()", self.actor.name());
        if self.message.is_some() {
            let actor = self.actor.take_actor().expect("actor is missing");
            let response = actor.on_request(self.as_mut().message.take().unwrap());

            match response {
                Response::Immediate(actor, val) => {
                    self.actor.replace_actor(actor);
                    return Poll::Ready(());
                }
                Response::ImmediateFuture(actor, fut) => {
                    self.actor.replace_actor(actor);
                    return Poll::Ready(());
                }
                defer @ Response::Defer(_) => {
                    self.defer.replace(defer);
                }
            }
        }

        if let Some(Response::Defer(ref mut fut)) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(response) => {
                    let actor = response.0;
                    self.actor.replace_actor(actor);
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

struct OnRequest<A>
where
    A: Actor + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<A::Request>,
    sender: CompletionSender<A::Response>,
    defer: Option<Response<A>>,
}

impl<A> OnRequest<A>
where
    A: Actor,
{
    pub fn new(
        actor: &'static ActorContext<A>,
        message: A::Request,
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

impl<A> OnRequest<A> where A: Actor + 'static {}

impl<A> ActorFuture<A> for OnRequest<A> where A: Actor {}

impl<A> Unpin for OnRequest<A> where A: Actor + 'static {}

impl<A> Future for OnRequest<A>
where
    A: Actor,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::trace!("[{}] Request.poll()", self.actor.name());
        if self.message.is_some() {
            let actor = self.actor.take_actor().expect("actor is missing");
            let response = actor.on_request(self.as_mut().message.take().unwrap());

            match response {
                Response::Immediate(actor, val) => {
                    self.actor.replace_actor(actor);
                    self.sender.send_value(val);
                    return Poll::Ready(());
                }
                Response::ImmediateFuture(actor, fut) => {
                    self.actor.replace_actor(actor);
                    self.sender.send_future(fut);
                    return Poll::Ready(());
                }
                defer @ Response::Defer(_) => {
                    self.defer.replace(defer);
                }
            }
        }

        if let Some(Response::Defer(ref mut fut)) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(response) => {
                    let actor = response.0;
                    self.actor.replace_actor(actor);
                    self.sender.send_value(response.1);
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
    panicking: Option<String<U32>>,
}

impl<R> Drop for RequestResponseFuture<R> {
    fn drop(&mut self) {
        if self.panicking.is_some() && !self.receiver.has_received() {
            panic!(
                "future must be .awaited: {}",
                self.panicking.as_ref().unwrap()
            )
        }
    }
}

impl<R> RequestResponseFuture<R> {
    fn new(receiver: CompletionReceiver<R>) -> Self {
        Self {
            receiver,
            panicking: None,
        }
    }

    fn new_panicking(receiver: CompletionReceiver<R>, debug: String<U32>) -> Self {
        Self {
            receiver,
            panicking: Some(debug),
        }
    }
}

impl<R> Future for RequestResponseFuture<R> {
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll(cx)
    }
}

struct CompletionHandle<T> {
    value: RefCell<Option<CompletionValue<T>>>,
    waker: RefCell<Option<Waker>>,
}

enum CompletionValue<T> {
    Immediate(T),
    Future(Box<dyn Future<Output = T>, SystemArena>),
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

    pub fn send_future(&self, value: Box<dyn Future<Output = T>, SystemArena>) {
        self.value
            .borrow_mut()
            .replace(CompletionValue::Future(value));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake()
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<T> {
        let mut value = self.value.borrow_mut();
        if value.is_none() {
            self.waker.borrow_mut().replace(cx.waker().clone());
            Poll::Pending
        } else {
            let mut v = value.take().unwrap();
            match v {
                CompletionValue::Immediate(val) => Poll::Ready(val),
                CompletionValue::Future(ref mut fut) => {
                    //log::info!("poll immediate future");
                    let fut = Pin::new(fut);
                    let result = fut.poll(cx);
                    if let Poll::Pending = result {
                        //log::info!("immediate_future is pending");
                        //self.waker.borrow_mut().replace(cx.waker().clone());
                        value.replace(v);
                    }
                    result
                }
            }
            //Poll::Ready(self.value.borrow_mut().take().unwrap())
        }
    }
}

struct CompletionSender<T: 'static> {
    handle: Rc<CompletionHandle<T>, SystemArena>,
    sent: AtomicBool,
}

impl<T: 'static> CompletionSender<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>, SystemArena>) -> Self {
        Self {
            handle,
            sent: AtomicBool::new(false),
        }
    }

    pub(crate) fn send_value(&self, response: T)
    where
        T: 'static,
    {
        self.sent.store(true, Ordering::Release);
        self.handle.send_value(response);
    }

    pub(crate) fn send_future(&self, response: Box<dyn Future<Output = T>, SystemArena>)
    where
        T: 'static,
    {
        self.sent.store(true, Ordering::Release);
        self.handle.send_future(response);
    }
}

struct CompletionReceiver<T: 'static> {
    handle: Rc<CompletionHandle<T>, SystemArena>,
    received: bool,
}

impl<T: 'static> CompletionReceiver<T> {
    pub(crate) fn new(handle: Rc<CompletionHandle<T>, SystemArena>) -> Self {
        Self {
            handle,
            received: false,
        }
    }

    pub(crate) fn has_received(&self) -> bool {
        self.received
    }

    pub(crate) fn poll(&mut self, cx: &mut Context) -> Poll<T>
    where
        T: 'static,
    {
        let result = self.handle.poll(cx);

        if let Poll::Ready(_) = result {
            self.received = true;
        }

        result
    }
}
