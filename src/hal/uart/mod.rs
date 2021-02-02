#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

pub trait Uart {
    /// Start a write operation to transmit the provided buffer.
    fn write_start(&mut self, tx_buffer: &[u8]) -> Result<(), Error>;

    /// Check progress of a write operation.
    fn write_done(&self) -> bool;

    /// Complete a write operation.
    fn write_finish(&mut self) -> Result<(), Error>;

    /// Process interrupts for the peripheral. Implementations may need to use this to initiate
    /// the next block of byte(s) to transfer.
    fn process_interrupt(&mut self);

    /// Start a read operation to receive data into rx_buffer.
    fn read_start(&mut self, rx_buffer: &mut [u8]) -> Result<(), Error>;

    /// Check progress of a write operation.
    fn read_done(&self) -> bool;

    /// Complete a read operation.
    fn read_finish(&mut self) -> Result<usize, Error>;

    /// Cancel a read operation
    fn read_cancel(&mut self) -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub enum Error {
    TxInProgress,
    RxInProgress,
    TxBufferTooSmall,
    RxBufferTooSmall,
    TxBufferTooLong,
    RxBufferTooLong,
    Transmit,
    Receive,
    Timeout(usize),
    BufferNotInRAM,
}
