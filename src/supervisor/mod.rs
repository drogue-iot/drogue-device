//! Opaque supervisor for internal operation.

use crate::platform::with_critical_section;
use crate::supervisor::actor_executor::{ActiveActor, ActorExecutor};
use crate::supervisor::interrupt_dispatcher::{ActiveInterrupt, InterruptDispatcher};
use core::cell::RefCell;

pub(crate) mod actor_executor;
pub(crate) mod interrupt_dispatcher;

use crate::define_arena;

define_arena!(SYSTEM);

/// An opaque object used during the mounting of actors into the system.
pub struct Supervisor {
    executor: RefCell<ActorExecutor>,
    dispatcher: RefCell<InterruptDispatcher>,
}

impl Supervisor {
    #[doc(hidden)]
    pub fn new() -> Self {
        Self {
            executor: RefCell::new(ActorExecutor::new()),
            dispatcher: RefCell::new(InterruptDispatcher::new()),
        }
    }

    pub(crate) fn activate_actor<S: ActiveActor>(
        &mut self,
        actor: &'static S,
    ) -> (usize, *const ()) {
        self.executor.borrow_mut().activate_actor(actor)
    }

    pub(crate) fn activate_interrupt<I: ActiveInterrupt>(
        &mut self,
        interrupt: &'static I,
        irq: u8,
    ) {
        self.dispatcher
            .borrow_mut()
            .activate_interrupt(interrupt, irq);
    }

    pub(crate) fn run_forever(&self) -> ! {
        with_critical_section(|cs| {
            self.dispatcher.borrow().unmask_all();
        });
        self.executor.borrow_mut().run_forever()
    }

    pub(crate) fn on_interrupt(&self, irqn: i16) {
        //log::info!("[supervisor] on IRQ {}", irqn);
        self.dispatcher.borrow().on_interrupt(irqn);
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Supervisor::new()
    }
}
