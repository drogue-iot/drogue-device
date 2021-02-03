#[cfg(any(
    feature = "nrf52832",
    feature = "nrf52833",
    feature = "nrf52840",
    feature = "nrf9160"
))]
pub mod nrf;

pub trait Uart {
    /// Start a write operation to transmit the provided buffer. Implementations can return
    /// TxBufferTooLong if buffer is too big.
    fn start_write(&self, tx_buffer: &[u8]) -> Result<(), Error>;

    /// Complete a write operation.
    fn finish_write(&self) -> Result<(), Error>;

    /// Cancel a write operation.
    fn cancel_write(&self);

    /// Start a read operation to receive data into rx_buffer. Implementations can return
    /// RxBufferTooLong if buffer is too big for implementations.
    fn start_read(&self, rx_buffer: &mut [u8]) -> Result<(), Error>;

    /// Complete a read operation.
    fn finish_read(&self) -> Result<usize, Error>;

    /// Cancel a read operation
    fn cancel_read(&self);

    /// Process interrupts for the peripheral.
    fn process_interrupts(&self) -> (bool, bool);
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
