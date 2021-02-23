use crate::hal::gpio::InterruptPin;
use embedded_hal::serial::Read;

impl<P> InterruptPin for P
where
    P: Read<u8>,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        true
    }
    fn clear_interrupt(&mut self) {}
}
