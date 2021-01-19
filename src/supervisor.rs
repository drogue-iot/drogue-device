use heapless::{
    Vec,
    consts::*,
};

use crate::actor::{Actor, ActorContext};
use core::task::{Poll, Context, Waker, RawWaker, RawWakerVTable};
use core::sync::atomic::{AtomicU8, Ordering};
use crate::interrupt::Interrupt;


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
        log::info!("my handle {:x}", &self.state as *const _ as u32);
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
        //log::info!(" {:x} --> {}", (&self.state ) as * const _ as u32, self.state.load( Ordering::Acquire) as u8);
        self.state.load(Ordering::Acquire) == ActorState::READY.into()
    }

    fn signal_ready(&self) {
        self.state.store(ActorState::READY.into(), Ordering::Release)
    }

    fn poll(&mut self) -> bool {
        if self.is_ready() {
            log::info!("actor is ready" );
            //self.signal_idle();
            //log::info!("actor signalling idle pre" );
            match self.actor.do_poll(self.get_state_flag_handle()) {
                Poll::Ready(_) => {
                    log::info!("actor signalling idle actual" );
                    // this is actually stopped
                    self.signal_idle()
                }
                Poll::Pending => {
                    log::info!("actor signalling waiting actual" );
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
                //log::info!("items: {}, current={}", (&*self.items.get()).len(), (&*self.current.get()).is_some());
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
                            log::info!("current complete, clearing" );
                            (&mut *self.current.get()).take();
                        }
                        Poll::Pending => {
                            log::info!("current pending, breaking" );
                            break;
                        }
                    }
                } else {
                    log::info!("no current");
                    break;
                }
            }
        }

        Poll::Pending
    }
}

pub(crate) trait ActiveInterrupt {
    fn on_interrupt(&self);
}

impl<I: Actor + Interrupt> ActiveInterrupt for ActorContext<I> {
    fn on_interrupt(&self) {
        log::info!( "--->");
        unsafe {
            (&mut *self.actor.get()).on_interrupt();
        }
    }
}

struct Interruptable {
    irq: u8,
    interrupt: &'static dyn ActiveInterrupt,
}

impl Interruptable {
    pub fn new(interrupt: &'static dyn ActiveInterrupt, irq: u8) -> Self {
        Self {
            irq,
            interrupt,
        }
    }
}

pub struct Supervisor {
    actors: Vec<Supervised, U16>,
    interrupts: Vec<Interruptable, U16>,
}

impl Supervisor {
    pub(crate) fn new() -> Self {
        Self {
            actors: Vec::new(),
            interrupts: Vec::new(),
        }
    }

    pub(crate) fn activate_actor<S: ActiveActor>(&mut self, actor: &'static S) -> *const () {
        let supervised = Supervised::new(actor);
        self.actors.push(supervised).unwrap_or_else( |_| panic!("too many actors" ) );
        self.actors[self.actors.len()-1].get_state_flag_handle()
    }

    pub(crate) fn activate_interrupt<I: ActiveInterrupt>(&mut self, interrupt: &'static I, irq: u8) {
        self.interrupts.push(Interruptable::new(interrupt, irq)).unwrap_or_else( |_| panic!( "too many interrupts" ) );
    }

    pub(crate) fn run_until_quiescence(&mut self) {
        //log::info!("run until quiescence");
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

    #[doc(hidden)]
    pub fn on_interrupt(&self, irqn: i16) {
        for interrupt in self.interrupts.iter().filter(|e| e.irq == irqn as u8) {
            log::info!("send along irq");
            interrupt.interrupt.on_interrupt();
        }
    }
}

// NOTE `*const ()` is &AtomicU8
static VTABLE: RawWakerVTable = {
    unsafe fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    unsafe fn wake(p: *const ()) {
        log::info!("wake");
        wake_by_ref(p)
    }

    unsafe fn wake_by_ref(p: *const ()) {
        log::info!("wake by ref");
        (*(p as *const AtomicU8)).store(ActorState::READY.into(), Ordering::Release);
    }

    unsafe fn drop(_: *const ()) {}

    RawWakerVTable::new(clone, wake, wake_by_ref, drop)
};