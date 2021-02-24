use crate::api::uart::Error;

/// Trait for devices that support UART as a interrupt-driven DMA peripheral.
pub trait DmaUartHal {
    /// Enable interrupts for TX and RX for the peripheral..
    fn enable_interrupt(&self);

    /// Prepare a write operation to transmit the provided buffer. Implementations can return
    /// TxBufferTooLong if buffer is too big.
    fn prepare_write(&self, tx_buffer: &[u8]) -> Result<(), Error>;

    /// Start DMA write operation
    fn start_write(&self);

    /// Complete a write operation.
    fn finish_write(&self) -> Result<(), Error>;

    /// Cancel a write operation.
    fn cancel_write(&self);

    /// Prepare a read operation to receive data into rx_buffer. This ensures that DMA registers
    /// are pointing to the provided buffer.
    fn prepare_read(&self, rx_buffer: &mut [u8]) -> Result<(), Error>;

    /// Initiate DMA read operation
    fn start_read(&self);

    /// Complete a read operation.
    fn finish_read(&self) -> usize;

    /// Cancel a read operation
    fn cancel_read(&self);

    /// Process interrupts for the peripheral. Returns booleans indicating (tx_done, rx_done).
    fn process_interrupts(&self) -> (bool, bool);
}
