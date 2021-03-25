use stm32f4xx_hal::gpio::ExtiPin;

use crate::hal::gpio::InterruptPin;

impl<P> InterruptPin for P
where
    P: ExtiPin,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        ExtiPin::check_interrupt(self)
    }

    fn clear_interrupt(&mut self) {
        ExtiPin::clear_interrupt_pending_bit(self)
    }
}
