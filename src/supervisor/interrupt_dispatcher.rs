use heapless::{consts::*, Vec};

use crate::actor::{Actor, ActorContext};
use crate::interrupt::{Interrupt, InterruptContext};
use core::sync::atomic::{AtomicU8, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub(crate) trait ActiveInterrupt {
    fn on_interrupt(&self);
}

impl<I: Actor + Interrupt> ActiveInterrupt for InterruptContext<I> {
    fn on_interrupt(&self) {
        // Mask this interrupt handler (not the entire IRQ) if this
        // actor currently has an in-flight async block.
        //
        // This does indeed mean that this interrupt will never be
        // processed. Perhaps we should just queue it up and deliver
        // after the async block has completed?
        if !self.actor_context.in_flight.load(Ordering::Acquire) {
            unsafe {
                //cortex_m::interrupt::free(|cs| {
                    (&mut *self.actor_context.actor.get()).on_interrupt();
                //})
            }
        }
    }
}

struct Interruptable {
    irq: u8,
    interrupt: &'static dyn ActiveInterrupt,
}

impl Interruptable {
    pub fn new(interrupt: &'static dyn ActiveInterrupt, irq: u8) -> Self {
        Self { irq, interrupt }
    }
}

pub struct InterruptDispatcher {
    interrupts: Vec<Interruptable, U16>,
}

impl InterruptDispatcher {
    pub(crate) fn new() -> Self {
        Self {
            interrupts: Vec::new(),
        }
    }

    pub(crate) fn activate_interrupt<I: ActiveInterrupt>(
        &mut self,
        interrupt: &'static I,
        irq: u8,
    ) {
        self.interrupts
            .push(Interruptable::new(interrupt, irq))
            .unwrap_or_else(|_| panic!("too many interrupts"));
    }

    #[doc(hidden)]
    pub(crate) fn on_interrupt(&self, irqn: i16) {
        for interrupt in self.interrupts.iter().filter(|e| e.irq == irqn as u8) {
            interrupt.interrupt.on_interrupt();
        }
    }
}
