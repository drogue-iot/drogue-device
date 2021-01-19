use crate::supervisor::Supervisor;
use core::cell::UnsafeCell;
use cortex_m::peripheral::NVIC;
use cortex_m::interrupt::Nr;
use crate::actor::{Actor, ActorContext};
use core::marker::PhantomData;
use crate::address::Address;

pub trait Interrupt: Actor {
    fn on_interrupt(&mut self);
}

pub struct InterruptContext<I: Interrupt> {
    actor_context: ActorContext<I>
}

impl<I:Interrupt> InterruptContext<I> {
    pub fn new<N:Nr>(interrupt: I, irq: N) -> Self {
        Self {
            actor_context: ActorContext::new_interrupt(interrupt, irq.nr())
        }
    }

    pub fn start(&'static self, supervisor: &mut Supervisor) -> Address<I> {
        log::info!("starting IRQ");
        self.actor_context.start_interrupt(supervisor)
    }

}

