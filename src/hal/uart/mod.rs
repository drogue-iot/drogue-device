#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

pub trait Uart {
    /// Start a write operation to transmit the provided buffer.
    fn write_start(&self, tx_buffer: &[u8]) -> Result<(), Error>;

    /// Complete a write operation.
    fn write_finish(&self) -> Result<(), Error>;

    /// Process interrupts for the peripheral. Implementations may need to use this to initiate
    fn process_interrupts(&self) -> (bool, bool);

    /// Start a read operation to receive data into rx_buffer.
    fn read_start(&self, rx_buffer: &mut [u8]) -> Result<(), Error>;

    /// Complete a read operation.
    fn read_finish(&self) -> Result<usize, Error>;

    /// Cancel a read operation
    fn read_cancel(&self) -> Result<(), Error>;
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
