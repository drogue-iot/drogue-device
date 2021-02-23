pub mod dma;

pub trait UartRx {
    fn enable_interrupt(&mut self);
    fn check_interrupt(&mut self) -> bool;
    fn clear_interrupt(&mut self);
}
