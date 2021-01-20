use crate::supervisor::actor_executor::{ActiveActor, ActorExecutor};
use crate::supervisor::interrupt_dispatcher::{InterruptDispatcher, ActiveInterrupt};
use core::cell::UnsafeCell;

pub(crate) mod actor_executor;
pub(crate) mod interrupt_dispatcher;

pub struct Supervisor {
    executor: UnsafeCell<ActorExecutor>,
    dispatcher: UnsafeCell<InterruptDispatcher>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            executor: UnsafeCell::new(ActorExecutor::new()),
            dispatcher: UnsafeCell::new(InterruptDispatcher::new()),
        }
    }

    pub(crate) fn activate_actor<S: ActiveActor>(&mut self, actor: &'static S) -> *const () {
        unsafe {
            (&mut *self.executor.get()).activate_actor(actor)
        }
    }

    pub(crate) fn activate_interrupt<I: ActiveInterrupt>(&mut self, interrupt: &'static I, irq: u8) {
        unsafe {
            (&mut *self.dispatcher.get()).activate_interrupt(interrupt, irq);
        }
    }

    pub(crate) fn run_forever(&self) -> ! {
        unsafe {
            (&mut *self.executor.get()).run_forever()
        }
    }

    pub(crate) fn on_interrupt(&self, irqn: i16) {
        unsafe {
            (&*self.dispatcher.get()).on_interrupt(irqn);
        }
    }
}