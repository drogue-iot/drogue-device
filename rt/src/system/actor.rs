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
use crate::arena::Box;
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
    type Response: Default;
    //    type ResponseFuture: Future<Output = Self::Response>;

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

    fn on_request(self, message: Self::Request) -> Response<Self>
    where
        Self: 'static,
    {
        Response::immediate(self, Default::default())
    }

    fn on_notify(self, message: Self::Request) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::immediate(self)
    }
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

type ItemsProducer<A: Actor> = RefCell<Option<Producer<'static, ActorRequest<A>, U8>>>;
type ItemsConsumer<A: Actor> = RefCell<Option<Consumer<'static, ActorRequest<A>, U8>>>;

/// Struct which is capable of holding an `Actor` instance
/// and connects it to the actor system.
pub struct ActorContext<A: Actor + 'static> {
    pub(crate) actor: RefCell<Option<A>>,
    pub(crate) current: RefCell<Option<ActorRequest<A>>>,
    // Only an UnsafeCell instead of RefCell in order to maintain it's 'static nature when borrowed.
    pub(crate) items: UnsafeCell<Queue<ActorRequest<A>, U8>>,
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

        with_critical_section(|cs| {
            self.items_producer
                .borrow_mut()
                .as_mut()
                .unwrap()
                .enqueue(ActorRequest::new(
                    self,
                    ActorMessage::Lifecycle(event),
                    None,
                ))
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

        unsafe {
            with_critical_section(|cs| {
                self.items_producer
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .enqueue(ActorRequest::new(self, ActorMessage::Notify(message), None))
                    .unwrap_or_else(|_| panic!("message queue full"));
            });

            //let flag_ptr = (&*self.state_flag_handle.get()).unwrap() as *const AtomicU8;
            let flag_ptr = self.state_flag_handle.borrow_mut().unwrap() as *const AtomicU8;
            (*flag_ptr).store(ActorState::READY.into(), Ordering::Release);
        }
    }

    /// Dispatch an async request.
    pub(crate) async fn request(&'static self, message: A::Request) -> A::Response {
        let signal: CompletionHandle<A> = CompletionHandle::new();
        let sender = CompletionSender::new(unsafe {
            core::mem::transmute::<_, &'static CompletionHandle<A>>(&signal)
        });
        let receiver = CompletionReceiver::new(&signal);
        let response = RequestResponseFuture::new(receiver);

        unsafe {
            with_critical_section(|cs| {
                self.items_producer
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .enqueue(ActorRequest::new(
                        self,
                        ActorMessage::Request(message),
                        Some(sender),
                    ))
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
        let signal: CompletionHandle<A> = CompletionHandle::new();
        let receiver = CompletionReceiver::new(&signal);
        let sender = CompletionSender::new(unsafe {
            core::mem::transmute::<_, &'static CompletionHandle<A>>(&signal)
        });
        let response = RequestResponseFuture::new(receiver);

        unsafe {
            with_critical_section(|cs| {
                self.items_producer
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .enqueue(ActorRequest::new(
                        self,
                        ActorMessage::Request(message),
                        Some(sender),
                    ))
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
        let signal: CompletionHandle<A> = CompletionHandle::new();
        let sender = CompletionSender::new(unsafe {
            core::mem::transmute::<_, &'static CompletionHandle<A>>(&signal)
        });
        let receiver = CompletionReceiver::new(&signal);

        let debug = "unknown".into();
        //write!(debug, "{:?}", type_name::<M>()).unwrap();

        let response = RequestResponseFuture::new_panicking(receiver, debug);

        unsafe {
            with_critical_section(|cs| {
                self.items_producer
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .enqueue(ActorRequest::new(
                        self,
                        ActorMessage::Request(message),
                        Some(sender),
                    ))
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

impl<A: Actor> Unpin for ActorRequest<A> {}

impl<A: Actor> Future for ActorRequest<A> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.message.take() {
            Some(ActorMessage::Lifecycle(event)) => {
                log::trace!(
                    "[{}] Lifecycle.poll() - dispatch on_lifecycle {:?}",
                    self.actor.name(),
                    event
                );
                let actor = self.actor.take_actor().expect("actor is missing");
                let completion = match event {
                    Lifecycle::Initialize => actor.on_initialize(),
                    Lifecycle::Start => actor.on_start(),
                    Lifecycle::Stop => actor.on_stop(),
                    Lifecycle::Sleep => actor.on_sleep(),
                    Lifecycle::Hibernate => actor.on_hibernate(),
                };
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
                        self.defer.replace(ActorResponse::Completion(completion));
                    }
                }
            }
            Some(ActorMessage::Notify(message)) => {
                let actor = self.actor.take_actor().expect("actor is missing");
                let completion = actor.on_notify(message);
                match completion {
                    Completion::Immediate(actor) => {
                        self.actor.replace_actor(actor);
                        log::trace!("[{}] Notify.poll() - immediate: Ready", self.actor.name());
                        return Poll::Ready(());
                    }
                    Completion::Defer(_) => {
                        self.defer.replace(ActorResponse::Completion(completion));
                    }
                }
            }
            Some(ActorMessage::Request(message)) => {
                let actor = self.actor.take_actor().expect("actor is missing");
                let response = actor.on_request(message);
                match response {
                    Response::Immediate(actor, val) => {
                        self.actor.replace_actor(actor);
                        self.sender.as_ref().unwrap().send_value(val);
                        return Poll::Ready(());
                    }
                    Response::ImmediateFuture(actor, fut) => {
                        self.actor.replace_actor(actor);
                        self.sender.as_ref().unwrap().send_future(fut);
                        return Poll::Ready(());
                    }
                    defer @ Response::Defer(_) => {
                        self.defer.replace(ActorResponse::Response(defer));
                    }
                }
            }
            _ => {}
        }

        log::trace!("[{}] Request .poll() - check defer", self.actor.name(),);

        if let Some(ActorResponse::Completion(Completion::Defer(ref mut fut))) = &mut self.defer {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(actor) => {
                    log::trace!("[{}] Completion.poll() - defer: Ready", self.actor.name(),);
                    self.actor.replace_actor(actor);
                    //self.sender.send(response);
                    self.defer.take();
                    Poll::Ready(())
                }
                Poll::Pending => {
                    log::trace!("[{}] Completion.poll() - defer: Pending", self.actor.name(),);
                    Poll::Pending
                }
            }
        } else if let Some(ActorResponse::Response(Response::Defer(ref mut fut))) = &mut self.defer
        {
            let fut = Pin::new(fut);
            let result = fut.poll(cx);
            match result {
                Poll::Ready(response) => {
                    let actor = response.0;
                    self.actor.replace_actor(actor);
                    self.sender.as_ref().unwrap().send_value(response.1);
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

enum ActorMessage<A: Actor> {
    Lifecycle(Lifecycle),
    Request(A::Request),
    Notify(A::Request),
}

pub struct ActorRequest<A>
where
    A: Actor + 'static,
{
    actor: &'static ActorContext<A>,
    message: Option<ActorMessage<A>>,
    sender: Option<CompletionSender<'static, A>>,
    defer: Option<ActorResponse<A>>,
}

enum ActorResponse<A>
where
    A: Actor + 'static,
{
    Response(Response<A>),
    Completion(Completion<A>),
}

impl<A> ActorRequest<A>
where
    A: Actor,
{
    fn new(
        actor: &'static ActorContext<A>,
        message: ActorMessage<A>,
        sender: Option<CompletionSender<'static, A>>,
    ) -> Self {
        Self {
            actor,
            message: Some(message),
            sender,
            defer: None,
        }
    }
}

struct RequestResponseFuture<'a, A: Actor + 'static> {
    receiver: CompletionReceiver<'a, A>,
    panicking: Option<String<U32>>,
}

impl<'a, A: Actor> Drop for RequestResponseFuture<'a, A> {
    fn drop(&mut self) {
        if self.panicking.is_some() && !self.receiver.has_received() {
            panic!(
                "future must be .awaited: {}",
                self.panicking.as_ref().unwrap()
            )
        }
    }
}

impl<'a, A: Actor> RequestResponseFuture<'a, A> {
    fn new(receiver: CompletionReceiver<'a, A>) -> Self {
        Self {
            receiver,
            panicking: None,
        }
    }

    fn new_panicking(receiver: CompletionReceiver<'a, A>, debug: String<U32>) -> Self {
        Self {
            receiver,
            panicking: Some(debug),
        }
    }
}

impl<'a, A: Actor> Future for RequestResponseFuture<'a, A> {
    type Output = A::Response;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.poll(cx)
    }
}

struct CompletionHandle<A: Actor> {
    value: RefCell<Option<CompletionValue<A::Response>>>,
    waker: RefCell<Option<Waker>>,
}

enum CompletionValue<T> {
    Immediate(T),
    Future(Box<dyn Future<Output = T>, SystemArena>),
}

impl<A: Actor> CompletionHandle<A> {
    pub fn new() -> Self {
        Self {
            value: RefCell::new(None),
            waker: RefCell::new(None),
        }
    }
}

impl<A: Actor> Default for CompletionHandle<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Actor + 'static> CompletionHandle<A> {
    pub fn send_value(&self, value: A::Response) {
        self.value
            .borrow_mut()
            .replace(CompletionValue::Immediate(value));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake()
        }
    }

    pub fn send_future(&self, value: Box<dyn Future<Output = A::Response>, SystemArena>) {
        self.value
            .borrow_mut()
            .replace(CompletionValue::Future(value));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake()
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<A::Response> {
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

struct CompletionSender<'a, A: Actor + 'static> {
    handle: &'a CompletionHandle<A>,
    sent: AtomicBool,
}

impl<'a, A: Actor + 'static> CompletionSender<'a, A> {
    pub(crate) fn new(handle: &'a CompletionHandle<A>) -> Self {
        Self {
            handle,
            sent: AtomicBool::new(false),
        }
    }

    pub(crate) fn send_value(&self, response: A::Response) {
        self.sent.store(true, Ordering::Release);
        self.handle.send_value(response);
    }

    pub(crate) fn send_future(&self, response: Box<dyn Future<Output = A::Response>, SystemArena>) {
        self.sent.store(true, Ordering::Release);
        self.handle.send_future(response);
    }
}

struct CompletionReceiver<'a, A: Actor + 'static> {
    handle: &'a CompletionHandle<A>,
    received: bool,
}

impl<'a, A: Actor + 'static> CompletionReceiver<'a, A> {
    pub(crate) fn new(handle: &'a CompletionHandle<A>) -> Self {
        Self {
            handle,
            received: false,
        }
    }

    pub(crate) fn has_received(&self) -> bool {
        self.received
    }

    pub(crate) fn poll(&mut self, cx: &mut Context) -> Poll<A::Response> {
        let result = self.handle.poll(cx);

        if let Poll::Ready(_) = result {
            self.received = true;
        }

        result
    }
}
