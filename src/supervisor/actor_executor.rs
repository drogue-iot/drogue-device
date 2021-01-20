use heapless::{
    Vec,
    consts::*,
};

use crate::actor::{Actor, ActorContext};
use core::task::{Poll, Context, Waker, RawWaker, RawWakerVTable};
use core::sync::atomic::{AtomicU8, Ordering};


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
        self.state.store(ActorState::IDLE.into(), Ordering::Release)
    }

    fn is_waiting(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::WAITING.into()
    }

    fn signal_waiting(&self) {
        self.state.store(ActorState::WAITING.into(), Ordering::Release)
    }

    fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::READY.into()
    }

    fn signal_ready(&self) {
        self.state.store(ActorState::READY.into(), Ordering::Release)
    }

    fn poll(&mut self) -> bool {
        if self.is_ready() {
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
    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()>;
}

impl<A: Actor> ActiveActor for ActorContext<A> {
    fn do_poll(&self, state_flag_handle: *const ()) -> Poll<()> {
        loop {
            unsafe {
                if (&*self.current.get()).is_none() {
                    if let Some(next) = (&mut *self.items.get()).dequeue() {
                        (&mut *self.current.get()).replace(next );
                    }
                }

                if let Some(item) = &mut *self.current.get() {
                    let raw_waker = RawWaker::new(state_flag_handle, &VTABLE);
                    let waker = Waker::from_raw(raw_waker);
                    let mut cx = Context::from_waker(&waker);

                    let result = item.poll(&mut cx);
                    match result {
                        Poll::Ready(_) => {
                            // "dequeue" it and allow it to drop
                            (&mut *self.current.get()).take();
                        }
                        Poll::Pending => {
                            break;
                        }
                    }
                } else {
                    break;
                }
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
        self.actors.push(supervised).unwrap_or_else( |_| panic!("too many actors" ) );
        self.actors[self.actors.len()-1].get_state_flag_handle()
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
        (*(p as *const AtomicU8)).store(ActorState::READY.into(), Ordering::Release);
    }

    unsafe fn drop(_: *const ()) {}

    RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};