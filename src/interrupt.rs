use crate::supervisor::Supervisor;
use cortex_m::interrupt::Nr;
use crate::actor::{Actor, ActorContext};
use crate::address::Address;

pub trait Interrupt: Actor {
    fn on_interrupt(&mut self);
}

pub struct InterruptContext<I: Interrupt> {
    actor_context: ActorContext<I>
}

impl<I:Interrupt> InterruptContext<I> {
    pub fn new<N:Nr>(interrupt: I, irq: N, name: &'static str) -> Self {
        Self {
            actor_context: ActorContext::new_interrupt(interrupt, irq.nr(), name)
        }
    }

    pub fn start(&'static self, supervisor: &mut Supervisor) -> Address<I> {
        log::info!("starting IRQ");
        self.actor_context.start_interrupt(supervisor)
    }

}

