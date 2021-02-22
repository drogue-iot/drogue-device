use heapless::{consts::*, Vec};

use crate::actor::{Actor, ActorContext, CURRENT};
use crate::prelude::device::Lifecycle;
use core::cmp::PartialEq;
use core::sync::atomic::{AtomicU8, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(PartialEq)]
pub(crate) enum ActorState {
    //IDLE = 0,
    WAITING = 0,
    READY = 1,
    //UNKNOWN = 127,
}

impl Into<u8> for ActorState {
    fn into(self) -> u8 {
        self as u8
    }
}

struct Supervised {
    actor: &'static dyn ActiveActor,
    state: AtomicU8,
}

impl Supervised {
    fn new<A: ActiveActor>(actor: &'static A) -> Self {
        Self {
            actor,
            state: AtomicU8::new(ActorState::READY.into()),
        }
    }

    fn get_state_flag_handle(&self) -> *const () {
        &self.state as *const _ as *const ()
    }

    fn is_waiting(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::WAITING as u8
    }

    fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) >= ActorState::READY as u8
    }

    fn decrement_ready(&self) {
        self.state.fetch_sub(1, Ordering::Acquire);
    }

    fn poll(&mut self) -> bool {
        if self.actor.name() == "uart_actor" || self.actor.name() == "rak811_ingress" {
            log::trace!("[{}] is ready {}", self.actor.name(), self.is_ready());
        }
        if self.is_ready() {
            log::trace!("polling actor {}", self.actor.name());
            match self.actor.do_poll(self.get_state_flag_handle()) {
                //Poll::Ready(_) => self.signal_idle(),
                Poll::Ready(_) => {}
                //Poll::Pending => self.signal_waiting(),
                Poll::Pending => self.decrement_ready(),
            }

            true
        } else {
            false
        }
    }

    fn dispatch_lifecycle_event(&self, event: Lifecycle) {
        self.actor.dispatch_lifecycle_event(event);
    }
}

pub(crate) trait ActiveActor {
    fn name(&self) -> &str;
    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()>;
    fn dispatch_lifecycle_event(&'static self, event: Lifecycle);
}

impl<A: Actor> ActiveActor for ActorContext<A> {
    fn name(&self) -> &str {
        ActorContext::name(self)
    }

    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()> {
        if self.name() == "uart_actor" {
            log::trace!("[{}] executor: do_poll", self.name());
        }
        unsafe {
            CURRENT.name.replace(self.name());
        }
        if self.name() == "uart_actor" {
            log::trace!("[{}] Replaced name", self.name());
        }
        loop {
            if self.current.borrow().is_none() {
                if let Some(next) = self.items_consumer.borrow_mut().as_mut().unwrap().dequeue() {
                    if self.name() == "uart_actor" {
                        log::trace!("[{}] executor: set current task", self.name());
                    }
                    self.current.borrow_mut().replace(next);
                    self.in_flight.store(true, Ordering::Release);
                } else {
                    if self.name() == "uart_actor" {
                        log::trace!("[{}] executor: no current task", self.name());
                    }
                    self.in_flight.store(false, Ordering::Release);
                }
            } else {
                if self.name() == "uart_actor" {
                    log::trace!("[{}] executor: in-flight current task", self.name());
                }
            }

            let should_drop;
            if let Some(item) = &mut *self.current.borrow_mut() {
                //&mut *self.current.get() {
                let raw_waker = RawWaker::new(state_flag_handle, &VTABLE);
                let waker = unsafe { Waker::from_raw(raw_waker) };
                let mut cx = Context::from_waker(&waker);

                let result = item.poll(&mut cx);
                match result {
                    Poll::Ready(_) => {
                        if self.name() == "uart_actor" {
                            log::trace!("[{}] executor: task complete", self.name());
                        }
                        should_drop = true;
                    }
                    Poll::Pending => {
                        if self.name() == "uart_actor" {
                            log::trace!("[{}] executor: task pending", self.name());
                        }
                        break;
                    }
                }
            } else {
                break;
            }
            if should_drop {
                let task = self.current.borrow_mut().take().unwrap();
                if self.name() == "uart_actor" {
                    log::trace!("[{}] executor: task drop", self.name());
                }
            }
        }

        unsafe {
            CURRENT.name.take();
        }

        Poll::Pending
    }

    fn dispatch_lifecycle_event(&'static self, event: Lifecycle) {
        self.lifecycle(event)
    }
}

pub struct ActorExecutor {
    actors: Vec<Supervised, U32>,
}

impl ActorExecutor {
    pub(crate) fn new() -> Self {
        Self { actors: Vec::new() }
    }

    pub(crate) fn dispatch_lifecycle_event(&mut self, event: Lifecycle) {
        //for actor in self.actors.iter().filter(|e| !e.is_idle()) {
        for actor in self.actors.iter() {
            actor.dispatch_lifecycle_event(event);
        }
    }

    pub(crate) fn activate_actor<S: ActiveActor>(
        &mut self,
        actor: &'static S,
    ) -> (usize, *const ()) {
        let supervised = Supervised::new(actor);
        self.actors
            .push(supervised)
            .unwrap_or_else(|_| panic!("too many actors"));
        log::trace!(
            "{} {:x}",
            actor.name(),
            (self.actors[self.actors.len() - 1].get_state_flag_handle() as u32)
        );
        (
            self.actors.len() - 1,
            self.actors[self.actors.len() - 1].get_state_flag_handle(),
        )
    }

    pub(crate) fn run_until_quiescence(&mut self) {
        let mut run_again = true;
        while run_again {
            run_again = false;
            //for actor in self.actors.iter_mut().filter(|e| !e.is_idle()) {
            for actor in self.actors.iter_mut() {
                if actor.poll() {
                    run_again = true
                }
            }
        }
    }

    pub fn run_forever(&mut self) -> ! {
        self.dispatch_lifecycle_event(Lifecycle::Initialize);
        self.run_until_quiescence();
        self.dispatch_lifecycle_event(Lifecycle::Start);
        loop {
            self.run_until_quiescence();
            // self.dispatch_lifecycle_event( Lifecycle::Sleep );
            // WFI
        }
    }
}

// NOTE `*const ()` is &AtomicU8
static VTABLE: RawWakerVTable = {
    unsafe fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    unsafe fn wake(p: *const ()) {
        wake_by_ref(p)
    }

    unsafe fn wake_by_ref(p: *const ()) {
        log::trace!("[waker] signal ready {:x}", p as *const _ as u32);
        //(*(p as *const AtomicU8)).store(ActorState::READY.into(), Ordering::Release);
        (*(p as *const AtomicU8)).fetch_add(1, Ordering::AcqRel);
    }

    unsafe fn drop(_: *const ()) {}

    RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};
