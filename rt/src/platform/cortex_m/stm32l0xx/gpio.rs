use crate::hal::gpio::InterruptPin;

impl<E> InterruptPin for E
where
    E: stm32l0xx_hal::exti::ExtiLine + Copy,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        stm32l0xx_hal::exti::Exti::is_pending(*self)
    }

    fn clear_interrupt(&mut self) {
        stm32l0xx_hal::exti::Exti::unpend(*self)
    }
}

impl<P, E> InterruptPin for Pin<P, E>
where
    P: embedded_hal::digital::v2::InputPin,
    E: stm32l0xx_hal::exti::ExtiLine + Copy,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        stm32l0xx_hal::exti::Exti::is_pending(self.line)
    }

    fn clear_interrupt(&mut self) {
        stm32l0xx_hal::exti::Exti::unpend(self.line)
    }
}

impl<P, E> embedded_hal::digital::v2::InputPin for Pin<P, E>
where
    P: embedded_hal::digital::v2::InputPin,
    E: stm32l0xx_hal::exti::ExtiLine + Copy,
{
    type Error = P::Error;

    fn is_high(&self) -> core::result::Result<bool, Self::Error> {
        self.pin.is_high()
    }

    fn is_low(&self) -> core::result::Result<bool, Self::Error> {
        self.pin.is_low()
    }
}

pub struct Pin<P, E>
where
    P: embedded_hal::digital::v2::InputPin,
    E: stm32l0xx_hal::exti::ExtiLine + Copy,
{
    pin: P,
    line: E,
}

impl<P, E> Pin<P, E>
where
    P: embedded_hal::digital::v2::InputPin,
    E: stm32l0xx_hal::exti::ExtiLine + Copy,
{
    pub fn new(pin: P, line: E) -> Self {
        Self { pin, line }
    }
}
