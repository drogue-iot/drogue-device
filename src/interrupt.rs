use crate::supervisor::Supervisor;
use core::cell::UnsafeCell;
use cortex_m::peripheral::NVIC;
use cortex_m::interrupt::Nr;

pub trait Interrupt: Sized {
    type StartArguments;

    fn irq(&self) -> u8;
    fn start(&mut self, args: Self::StartArguments);
    fn on_interrupt(&mut self);
}

pub struct InterruptContext<I:Interrupt> {
    pub(crate) interrupt: UnsafeCell<I>,
}

impl<I:Interrupt> InterruptContext<I> {

    pub fn new(interrupt: I) -> Self {
        Self {
            interrupt: UnsafeCell::new(interrupt),
        }
    }

    pub fn start(&'static self, args: I::StartArguments, supervisor: &mut Supervisor) {
        unsafe {
            (&mut *self.interrupt.get()).start(args);
        }
        supervisor.activate_interrupt(self);
        struct IrqNr(u8);
        unsafe impl Nr for IrqNr {
            fn nr(&self) -> u8 {
                self.0
            }
        }
        unsafe {
            NVIC::unmask( IrqNr( (&*self.interrupt.get()).irq() ) );
        }
    }

}
