use core::cell::UnsafeCell;
use cortex_m::interrupt::Nr;
use cortex_m::peripheral::NVIC;
use heapless::consts::U4;

use crate::actor::{Actor, ActorContext};
use crate::address::{Address, InterruptAddress};
use crate::handler::NotificationHandler;
use crate::sink::{Message, MultiSink, Sink};
use crate::supervisor::Supervisor;

pub trait Interrupt: Actor {
    type Event: Message;
    fn on_interrupt(&mut self, sink: &dyn Sink<Self::Event>);
}

pub struct InterruptContext<I: Interrupt> {
    pub(crate) subscribers: UnsafeCell<MultiSink<I::Event, U4>>,
    pub(crate) irq: u8,
    pub(crate) actor_context: ActorContext<I>,
}

impl<I: Interrupt> InterruptContext<I> {
    pub fn new<N: Nr>(interrupt: I, irq: N) -> Self {
        Self {
            irq: irq.nr(),
            actor_context: ActorContext::new(interrupt),
            subscribers: UnsafeCell::new(MultiSink::<_, U4>::new()),
        }
    }

    pub fn with_name(mut self, name: &'static str) -> Self {
        self.actor_context = self.actor_context.with_name(name);
        self
    }

    pub(crate) fn add_subscriber(&'static self, sub: &'static dyn Sink<I::Event>) {
        unsafe {
            (&mut *self.subscribers.get()).add(sub);
        }
    }

    pub fn start(&'static self, supervisor: &mut Supervisor) -> InterruptAddress<I> {
        let addr = self.actor_context.start(supervisor);
        supervisor.activate_interrupt(self, self.irq);

        struct IrqNr(u8);
        unsafe impl Nr for IrqNr {
            fn nr(&self) -> u8 {
                self.0
            }
        }
        unsafe { NVIC::unmask(IrqNr(self.irq)) }

        InterruptAddress::new(self, addr)
    }
}
