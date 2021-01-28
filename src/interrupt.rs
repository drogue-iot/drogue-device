use cortex_m::interrupt::Nr;
use cortex_m::peripheral::NVIC;

use crate::actor::{Actor, ActorContext};
use crate::address::Address;
use crate::bus::EventBus;
use crate::device::Device;
use crate::supervisor::Supervisor;

pub trait Interrupt<D: Device>: Actor<D> {
    fn on_interrupt(&mut self);
}

pub struct InterruptContext<D: Device, I: Interrupt<D>> {
    pub(crate) irq: u8,
    pub(crate) actor_context: ActorContext<D, I>,
}

impl<D: Device, I: Interrupt<D>> InterruptContext<D, I> {
    pub fn new<N: Nr>(interrupt: I, irq: N) -> Self {
        Self {
            irq: irq.nr(),
            actor_context: ActorContext::new(interrupt),
        }
    }

    pub fn with_name(mut self, name: &'static str) -> Self {
        self.actor_context = self.actor_context.with_name(name);
        self
    }

    pub fn mount(&'static self, bus: &EventBus<D>, supervisor: &mut Supervisor) -> Address<D, I> {
        let addr = self.actor_context.mount(bus, supervisor);
        supervisor.activate_interrupt(self, self.irq);

        struct IrqNr(u8);
        unsafe impl Nr for IrqNr {
            fn nr(&self) -> u8 {
                self.0
            }
        }
        log::info!("[irq] unmask {}", self.irq);
        unsafe { NVIC::unmask(IrqNr(self.irq)) }

        addr
    }
}
