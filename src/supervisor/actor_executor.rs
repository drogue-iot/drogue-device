use heapless::{
    Vec,
    consts::*,
};

use crate::actor::{Actor, ActorContext};
use core::task::{Poll, Context, Waker, RawWaker, RawWakerVTable};
use core::sync::atomic::{AtomicU8, Ordering};
use core::ops::Deref;


pub(crate) enum ActorState {
    IDLE = 0,
    WAITING = 1,
    READY = 2,
    UNKNOWN = 127,
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

    fn is_idle(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::IDLE.into()
    }

    fn signal_idle(&self) {
        log::trace!("[{}] signal idle {:x}", self.actor.name(), &self.state as * const _ as u32);
        self.state.store(ActorState::IDLE.into(), Ordering::Release)
    }

    fn is_waiting(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::WAITING.into()
    }

    fn signal_waiting(&self) {
        log::trace!("[{}] signal waiting {:x}", self.actor.name(), &self.state as * const _ as u32);
        self.state.store(ActorState::WAITING.into(), Ordering::Release)
    }

    fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::READY.into()
    }

    fn signal_ready(&self) {
        log::trace!("[{}] signal ready {:x}", self.actor.name(), &self.state as * const _ as u32);
        self.state.store(ActorState::READY.into(), Ordering::Release)
    }

    fn poll(&mut self) -> bool {
        if self.is_ready() {
            log::trace!("polling actor {:x}", &self.actor as *const _ as u32);
            match self.actor.do_poll(self.get_state_flag_handle()) {
                Poll::Ready(_) => {
                    self.signal_idle()
                }
                Poll::Pending => {
                    self.signal_waiting()
                }
            }
            true
        } else {
            false
        }
    }
}

pub(crate) trait ActiveActor {
    fn name(&self) -> &str;
    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()>;
}

impl<A: Actor> ActiveActor for ActorContext<A> {
    fn name(&self) -> &str {
        ActorContext::name(self)
    }

    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()> {
        log::trace!("[{}] executor: do_poll", self.name());
        loop {
            if self.current.borrow().is_none() {
                cortex_m::interrupt::free(|cs| {
                    if let Some(next) = self.items.borrow_mut().dequeue()  {
                        log::trace!("[{}] executor: set current task", self.name());
                        //(&mut *self.current.get()).replace(next);
                        self.current.borrow_mut().replace(next);
                        self.in_flight.store(true, Ordering::Release);
                    } else {
                        log::trace!("[{}] executor: no current task", self.name());
                        self.in_flight.store(false, Ordering::Release);
                    }
                });
            } else {
                log::trace!("[{}] executor: in-flight current task", self.name());
            }

            let mut should_drop = false;
            if let Some(item) = &mut *self.current.borrow_mut() { //&mut *self.current.get() {
                let raw_waker = RawWaker::new(state_flag_handle, &VTABLE);
                let waker = unsafe{ Waker::from_raw(raw_waker) };
                let mut cx = Context::from_waker(&waker);

                let result = item.poll(&mut cx);
                match result {
                    Poll::Ready(_) => {
                        log::trace!("[{}] executor: task complete", self.name());
                        should_drop = true;
                        // "dequeue" it and allow it to drop
                        //(&mut *self.current.get()).take();
                        //self.current.borrow_mut().take();
                    }
                    Poll::Pending => {
                        log::trace!("[{}] executor: task pending", self.name());
                        break;
                    }
                }
            } else {
                break;
            }
            if should_drop {
                log::trace!("[{}] executor: task drop", self.name());
                self.current.borrow_mut().take().unwrap();
            }
        }

        Poll::Pending
    }
}


pub struct ActorExecutor {
    actors: Vec<Supervised, U16>,
}

impl ActorExecutor {
    pub(crate) fn new() -> Self {
        Self {
            actors: Vec::new(),
        }
    }

    pub(crate) fn activate_actor<S: ActiveActor>(&mut self, actor: &'static S) -> *const () {
        let supervised = Supervised::new(actor);
        self.actors.push(supervised).unwrap_or_else(|_| panic!("too many actors"));
        self.actors[self.actors.len() - 1].get_state_flag_handle()
    }

    pub(crate) fn run_until_quiescence(&mut self) {
        let mut run_again = true;
        while run_again {
            run_again = false;
            for actor in self.actors.iter_mut().filter(|e| !e.is_idle()) {
                if actor.poll() {
                    run_again = true
                }
            }
        }
    }

    pub fn run_forever(&mut self) -> ! {
        loop {
            self.run_until_quiescence();
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
        (*(p as *const AtomicU8)).store(ActorState::READY.into(), Ordering::Release);
    }

    unsafe fn drop(_: *const ()) {}

    RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};