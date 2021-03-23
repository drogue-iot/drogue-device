use crate::hal::uart::UartRx;
use embedded_hal::serial::Read;

impl<P> UartRx for P
where
    P: Read<u8>,
{
    fn enable_interrupt(&mut self) {}
    fn check_interrupt(&mut self) -> bool {
        true
    }
    fn clear_interrupt(&mut self) {}
}
