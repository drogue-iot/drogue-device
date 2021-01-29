//! Types and traits for interrupt-capable actors.


use cortex_m::interrupt::Nr;
use cortex_m::peripheral::NVIC;

use crate::actor::{Actor, ActorContext};
use crate::address::Address;
use crate::supervisor::Supervisor;

/// Additional trait applicable to `Actor`s indicating their ability
/// to respond to hardware interrupts.
pub trait Interrupt: Actor {

    /// Process the interrupt.
    ///
    /// This method operates within the context of the actor, and
    /// during its processing the actor cannot respond to other
    /// requests or notifications.
    ///
    /// Additionally, while the actor _is_ responding to other
    /// requests or notifications, the associated interrupt will
    /// be masked for _this_ specific actor.
    fn on_interrupt(&mut self);
}

/// Struct which is capable of holding an `Interrupt` actor instance
/// and connecting it to the actor system.
pub struct InterruptContext<I: Interrupt> {
    pub(crate) irq: u8,
    pub(crate) actor_context: ActorContext<I>,
}

impl<I: Interrupt> InterruptContext<I> {

    /// Create a new context, taking ownership of the provided actor instance.
    /// When mounted, the context and the contained actor will be moved to the static lifetime.
    pub fn new<N: Nr>(interrupt: I, irq: N) -> Self {
        Self {
            irq: irq.nr(),
            actor_context: ActorContext::new(interrupt),
        }
    }

    /// Provide an optional name for the actor.
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.actor_context = self.actor_context.with_name(name);
        self
    }

    /// Mount the context and its actor into the system.
    pub fn mount(&'static self, supervisor: &mut Supervisor) -> Address<I> {
        let addr = self.actor_context.mount(supervisor);
        supervisor.activate_interrupt(self, self.irq);

        struct IrqNr(u8);
        unsafe impl Nr for IrqNr {
            fn nr(&self) -> u8 {
                self.0
            }
        }
        unsafe { NVIC::unmask(IrqNr(self.irq)) }

        addr
    }
}
