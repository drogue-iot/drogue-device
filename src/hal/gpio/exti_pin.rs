pub trait ExtiPin {
    fn check_interrupt(&mut self) -> bool;
    fn clear_interrupt_pending_bit(&mut self);
}

#[cfg(feature = "stm32l4xx")]
impl<P> ExtiPin for P
    where P: stm32l4xx_hal::gpio::ExtiPin
{
    fn check_interrupt(&mut self) -> bool {
        (self as &mut dyn stm32l4xx_hal::gpio::ExtiPin).check_interrupt()
    }

    fn clear_interrupt_pending_bit(&mut self) {
        stm32l4xx_hal::gpio::ExtiPin::clear_interrupt_pending_bit(self)
    }
}