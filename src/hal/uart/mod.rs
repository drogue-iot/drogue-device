#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

pub trait Uart {
    fn write_start(&mut self, tx_buffer: &[u8]) -> Result<(), Error>;
    fn write_done(&mut self) -> bool;
    fn write_finish(&mut self) -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub enum Error {
    TxInProgress,
    TxBufferTooSmall,
    RxBufferTooSmall,
    TxBufferTooLong,
    RxBufferTooLong,
    Transmit,
    Receive,
    Timeout(usize),
    BufferNotInRAM,
}
