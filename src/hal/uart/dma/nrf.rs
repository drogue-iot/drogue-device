//! Uart implementation for nRF series
#[cfg(feature = "nrf52833")]
use nrf52833_hal as hal;

#[allow(unused_imports)]
#[cfg(any(feature = "nrf52833", feature = "nrf52840"))]
use hal::pac::UARTE1;

#[cfg(feature = "nrf9160")]
use hal::pac::{uarte0_ns as uarte0, UARTE0_NS as UARTE0, UARTE1_NS as UARTE1};

#[cfg(not(feature = "nrf9160"))]
use hal::pac::{uarte0, UARTE0};

use crate::hal::uart::Error;
use core::ops::Deref;
use core::sync::atomic::{compiler_fence, Ordering::SeqCst};

pub use hal::uarte::{Baudrate, Parity, Pins};

pub struct Uarte<T>
where
    T: Instance,
{
    uart: T,
    pins: Pins,
}

impl<T> Uarte<T>
where
    T: Instance + hal::uarte::Instance,
{
    pub fn new(uart: T, pins: Pins, parity: Parity, baudrate: Baudrate) -> Self {
        let (uart, pins) = hal::uarte::Uarte::new(uart, pins, parity, baudrate).free();
        uart.inten
            .modify(|_, w| w.endrx().set_bit().endtx().set_bit());

        Self { uart, pins }
    }
}

impl<T> crate::hal::uart::dma::DmaUartHal for Uarte<T>
where
    T: Instance,
{
    fn start_write(&self, tx_buffer: &[u8]) -> Result<(), Error> {
        if tx_buffer.len() > hal::target_constants::EASY_DMA_SIZE {
            return Err(Error::TxBufferTooLong);
        }

        // We can only DMA out of RAM.
        slice_in_ram_or(tx_buffer, crate::hal::uart::Error::BufferNotInRAM)?;

        start_write(&*self.uart, tx_buffer);
        Ok(())
    }

    fn process_interrupts(&self) -> (bool, bool) {
        let tx_done = self.uart.events_endtx.read().bits() != 0
            || self.uart.events_txstopped.read().bits() != 0;

        let rx_done = self.uart.events_endrx.read().bits() != 0;

        if self.uart.events_error.read().bits() != 0 {
            self.uart.events_error.reset();
        }

        if self.uart.events_txstarted.read().bits() != 0 {
            self.uart.events_txstarted.reset();
        }

        if self.uart.events_txstopped.read().bits() != 0 {
            self.uart.events_txstopped.reset();
        }

        if self.uart.events_endtx.read().bits() != 0 {
            self.uart.events_endtx.reset();
        }

        if self.uart.events_endrx.read().bits() != 0 {
            self.uart.events_endrx.reset();
        }

        (tx_done, rx_done)
    }

    fn finish_write(&self) -> Result<(), Error> {
        // Conservative compiler fence to prevent optimizations that do not
        // take in to account actions by DMA. The fence has been placed here,
        // after all possible DMA actions have completed.
        compiler_fence(SeqCst);

        if self.uart.events_txstopped.read().bits() != 0 {
            return Err(Error::Transmit);
        }

        stop_write(&*self.uart);
        Ok(())
    }

    /// Cancel a write operation
    fn cancel_write(&self) {
        cancel_write(&*self.uart);
    }

    /// Start a read operation to receive data into rx_buffer.
    fn start_read(&self, rx_buffer: &mut [u8]) -> Result<(), Error> {
        slice_in_ram_or(rx_buffer, crate::hal::uart::Error::BufferNotInRAM)?;
        start_read(&*self.uart, rx_buffer)?;
        Ok(())
    }

    /// Complete a read operation.
    fn finish_read(&self) -> Result<usize, Error> {
        finalize_read(&*self.uart);

        let bytes_read = self.uart.rxd.amount.read().bits() as usize;

        Ok(bytes_read)
    }

    /// Cancel a read operation
    fn cancel_read(&self) {
        cancel_read(&*self.uart);
    }
}

// ---- Utilities copied from nrf-hal

/// Write via UARTE.
///
/// This method uses transmits all bytes in `tx_buffer`.
fn start_write(uarte: &uarte0::RegisterBlock, tx_buffer: &[u8]) {
    // Conservative compiler fence to prevent optimizations that do not
    // take in to account actions by DMA. The fence has been placed here,
    // before any DMA action has started.
    compiler_fence(SeqCst);

    // Reset the events.
    uarte.events_endtx.reset();
    uarte.events_txstopped.reset();

    // Set up the DMA write.
    uarte.txd.ptr.write(|w|
        // We're giving the register a pointer to the stack. Since we're
        // waiting for the UARTE transaction to end before this stack pointer
        // becomes invalid, there's nothing wrong here.
        //
        // The PTR field is a full 32 bits wide and accepts the full range
        // of values.
        unsafe { w.ptr().bits(tx_buffer.as_ptr() as u32) });
    uarte.txd.maxcnt.write(|w|
        // We're giving it the length of the buffer, so no danger of
        // accessing invalid memory. We have verified that the length of the
        // buffer fits in an `u8`, so the cast to `u8` is also fine.
        //
        // The MAXCNT field is 8 bits wide and accepts the full range of
        // values.
        unsafe { w.maxcnt().bits(tx_buffer.len() as _) });

    // Start UARTE Transmit transaction.
    uarte.tasks_starttx.write(|w|
        // `1` is a valid value to write to task registers.
        unsafe { w.bits(1) });
}

fn stop_write(uarte: &uarte0::RegisterBlock) {
    // `1` is a valid value to write to task registers.
    uarte.tasks_stoptx.write(|w| unsafe { w.bits(1) });

    // Wait for transmitter is stopped.
    while uarte.events_txstopped.read().bits() == 0 {}

    // Reset events
    uarte.events_endtx.reset();
    uarte.events_txstopped.reset();
}

/// Start a UARTE read transaction by setting the control
/// values and triggering a read task.
fn start_read(uarte: &uarte0::RegisterBlock, rx_buffer: &mut [u8]) -> Result<(), Error> {
    if rx_buffer.len() > hal::target_constants::EASY_DMA_SIZE {
        return Err(Error::RxBufferTooLong);
    }

    // NOTE: RAM slice check is not necessary, as a mutable slice can only be
    // built from data located in RAM.

    // Conservative compiler fence to prevent optimizations that do not
    // take in to account actions by DMA. The fence has been placed here,
    // before any DMA action has started.
    uarte.enable.write(|w| w.enable().enabled());

    compiler_fence(SeqCst);

    // Set up the DMA read
    uarte
        .rxd
        .ptr
        .write(|w| unsafe { w.ptr().bits(rx_buffer.as_ptr() as u32) });

    uarte
        .rxd
        .maxcnt
        .write(|w| unsafe { w.maxcnt().bits(rx_buffer.len() as _) });

    // Start UARTE Receive transaction.
    uarte.tasks_startrx.write(|w| unsafe { w.bits(1) });

    Ok(())
}

/// Stop an unfinished UART write transaction.
fn cancel_write(uarte: &uarte0::RegisterBlock) {
    stop_write(uarte);

    // Reset events
    uarte.events_endtx.reset();
    uarte.events_txstopped.reset();

    // Ensure the above is done
    compiler_fence(SeqCst);
}

/// Stop an unfinished UART read transaction and flush FIFO to DMA buffer.
fn cancel_read(uarte: &uarte0::RegisterBlock) {
    // Stop reception.
    uarte.tasks_stoprx.write(|w| unsafe { w.bits(1) });

    // Wait for the reception to have stopped.
    while uarte.events_rxto.read().bits() == 0 {}

    // Reset the event flag.
    uarte.events_rxto.write(|w| w);

    // Ask UART to flush FIFO to DMA buffer.
    uarte.tasks_flushrx.write(|w| unsafe { w.bits(1) });

    // Wait for the flush to complete.
    while uarte.events_endrx.read().bits() == 0 {}

    // The event flag itself is later reset by `finalize_read`.
}

/// Finalize a UARTE read transaction by clearing the event.
fn finalize_read(uarte: &uarte0::RegisterBlock) {
    // Reset the event, otherwise it will always read `1` from now on.
    uarte.events_endrx.write(|w| w);

    // Conservative compiler fence to prevent optimizations that do not
    // take in to account actions by DMA. The fence has been placed here,
    // after all possible DMA actions have completed.
    compiler_fence(SeqCst);
}

pub trait Instance: Deref<Target = uarte0::RegisterBlock> + sealed::Sealed {
    fn ptr() -> *const uarte0::RegisterBlock;
}

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for UARTE0 {}
impl Instance for UARTE0 {
    fn ptr() -> *const uarte0::RegisterBlock {
        UARTE0::ptr()
    }
}

#[cfg(any(feature = "52833", feature = "52840", feature = "9160"))]
mod _uarte1 {
    use super::*;
    impl sealed::Sealed for UARTE1 {}
    impl Instance for UARTE1 {
        fn ptr() -> *const uarte0::RegisterBlock {
            UARTE1::ptr()
        }
    }
}

/// Does this slice reside entirely within RAM?
fn slice_in_ram(slice: &[u8]) -> bool {
    let ptr = slice.as_ptr() as usize;
    ptr >= hal::target_constants::SRAM_LOWER
        && (ptr + slice.len()) < hal::target_constants::SRAM_UPPER
}

/// Return an error if slice is not in RAM.
#[cfg(not(feature = "51"))]
pub(crate) fn slice_in_ram_or<T>(slice: &[u8], err: T) -> Result<(), T> {
    if slice_in_ram(slice) {
        Ok(())
    } else {
        Err(err)
    }
}
