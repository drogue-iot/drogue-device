use crate::system::{
    actor::{Actor, ActorContext, ActorMessage, ActorState},
    signal::SignalSlot,
};
use core::sync::atomic::{AtomicU8, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use heapless::{consts, ArrayLength, Vec};

trait ActiveActor {
    fn is_ready(&self) -> bool;
    fn is_waiting(&self) -> bool;
    fn do_poll(&self);
}

impl<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>> ActiveActor
    for ActorContext<A, Q>
{
    fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) >= ActorState::READY as u8
    }

    fn is_waiting(&self) -> bool {
        self.state.load(Ordering::Acquire) == ActorState::WAITING as u8
    }

    fn do_poll(&self) {
        log::info!("[ActiveActor] do_poll()");
        if self.current.borrow().is_none() {
            if let Some(next) = self.next_message() {
                self.current.borrow_mut().replace(next);
                self.in_flight.store(true, Ordering::Release);
            } else {
                self.in_flight.store(false, Ordering::Release);
            }
        }

        if let Some(item) = &mut *self.current.borrow_mut() {
            let state_flag_handle = &self.state as *const _ as *const ();
            let raw_waker = RawWaker::new(state_flag_handle, &VTABLE);
            let waker = unsafe { Waker::from_raw(raw_waker) };
            let mut cx = Context::from_waker(&waker);

            let mut actor = self.actor.borrow_mut();
            let actor = actor.as_mut().unwrap();
            if let Poll::Ready(_) =
                actor.poll_message(unsafe { &mut **item.inner.get_mut() }, &mut cx)
            {
                unsafe { &**item.signal.borrow() }.signal()
            }
        }

        self.state.fetch_sub(1, Ordering::Acquire);
        log::info!(" and done {}", self.is_ready());
    }
}

unsafe impl Send for Supervised<'_> {}

pub struct ActorExecutor<'a> {
    actors: Vec<Supervised<'a>, consts::U16>,
}

struct Supervised<'a> {
    actor: &'a dyn ActiveActor,
}

impl<'a> Supervised<'a> {
    fn new<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>>(
        actor: &'a ActorContext<A, Q>,
    ) -> Self {
        Self { actor }
    }

    fn poll(&mut self) -> bool {
        if self.actor.is_ready() {
            self.actor.do_poll();
            true
        } else {
            false
        }
    }
}

impl<'a> ActorExecutor<'a> {
    pub fn new() -> Self {
        Self { actors: Vec::new() }
    }

    pub(crate) fn activate_actor<
        A: Actor,
        Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>,
    >(
        &mut self,
        actor: &'a ActorContext<A, Q>,
    ) {
        let supervised = Supervised::new(actor);
        self.actors
            .push(supervised)
            .unwrap_or_else(|_| panic!("too many actors"));
    }

    pub(crate) fn run_until_quiescence(&mut self) {
        let mut run_again = true;
        while run_again {
            run_again = false;
            for actor in self.actors.iter_mut() {
                if actor.poll() {
                    run_again = true
                }
            }
        }
    }

    pub fn run_forever(&mut self) -> ! {
        loop {
            self.run_until_quiescence();
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
        (&*(p as *const AtomicU8)).fetch_add(1, Ordering::AcqRel);
    }

    unsafe fn drop(_: *const ()) {}

    RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};
