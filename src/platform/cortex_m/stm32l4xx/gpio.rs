use crate::hal::gpio::InterruptPin;

impl<P> InterruptPin for P
where
    P: stm32l4xx_hal::gpio::ExtiPin,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        (self as &mut dyn stm32l4xx_hal::gpio::ExtiPin).check_interrupt()
    }

    fn clear_interrupt(&mut self) {
        stm32l4xx_hal::gpio::ExtiPin::clear_interrupt_pending_bit(self)
    }
}
