//! Uart implementation for nRF series
#[cfg(feature = "chip+nrf52833")]
use nrf52833_hal as hal;

#[allow(unused_imports)]
#[cfg(any(feature = "chip+nrf52833", feature = "chip+nrf52840"))]
use hal::pac::UARTE1;

#[cfg(feature = "chip+nrf9160")]
use hal::pac::{uarte0_ns as uarte0, UARTE0_NS as UARTE0, UARTE1_NS as UARTE1};

#[cfg(not(feature = "chip+nrf9160"))]
use hal::pac::uarte0;

use crate::api::uart::Error;
use atomic_polyfill::{compiler_fence, Ordering::SeqCst};

use crate::arch::with_critical_section;
use embedded_hal::serial;
pub use hal::uarte::{Baudrate, Instance, Parity, Pins};

pub struct Uarte<T>
where
    T: Instance,
{
    uarte: hal::uarte::Uarte<T>,
}

impl<T> Uarte<T>
where
    T: Instance,
{
    pub fn new(uart: T, pins: Pins, parity: Parity, baudrate: Baudrate) -> Self {
        let uarte = hal::uarte::Uarte::new(uart, pins, parity, baudrate);

        Self { uarte }
    }

    pub fn split(self, rx_buf: &'static mut [u8; 1]) -> (UarteTx<T>, UarteRx<T>) {
        let tx = UarteTx { uarte: self.uarte };
        let rx = UarteRx {
            rx_buf,
            _marker: core::marker::PhantomData,
        };
        (tx, rx)
    }
}

pub struct UarteTx<T>
where
    T: Instance,
{
    uarte: hal::uarte::Uarte<T>,
}

pub struct UarteRx<T>
where
    T: Instance,
{
    _marker: core::marker::PhantomData<T>,
    rx_buf: &'static mut [u8; 1],
}

impl<T> crate::hal::uart::UartRx for UarteRx<T>
where
    T: Instance,
{
    fn enable_interrupt(&mut self) {
        let uart = unsafe { &*T::ptr() };
        uart.inten
            .modify(|_, w| w.endrx().set_bit().rxto().set_bit());
        prepare_read(uart, &mut self.rx_buf[..]).unwrap();
        start_read(uart);
    }

    fn check_interrupt(&mut self) -> bool {
        let uart = unsafe { &*T::ptr() };
        uart.events_endrx.read().bits() != 0
    }

    fn clear_interrupt(&mut self) {
        let uart = unsafe { &*T::ptr() };
        prepare_read(uart, &mut self.rx_buf[..]).unwrap();
        start_read(uart);
    }
}

impl<T> serial::Write<u8> for UarteTx<T>
where
    T: Instance,
{
    type Error = Error;

    /// Write a single byte to the internal buffer. Returns nb::Error::WouldBlock if buffer is full.
    fn write(&mut self, b: u8) -> nb::Result<(), Self::Error> {
        let mut tx_buf = [0; 1];
        tx_buf[0] = b;
        self.uarte
            .write(&tx_buf[..])
            .map_err(|_| nb::Error::Other(Error::Transmit))
    }

    /// Flush the TX buffer non-blocking. Returns nb::Error::WouldBlock if not yet flushed.
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}

impl<T> serial::Read<u8> for UarteRx<T>
where
    T: Instance,
{
    type Error = Error;
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        let uarte = unsafe { &*T::ptr() };

        compiler_fence(SeqCst);

        let in_progress = uarte.events_rxstarted.read().bits() == 1;
        if in_progress && uarte.events_endrx.read().bits() == 0 {
            return Err(nb::Error::WouldBlock);
        }

        if in_progress {
            uarte.events_rxstarted.reset();

            finalize_read(uarte);

            if uarte.rxd.amount.read().bits() != 1 as u32 {
                return Err(nb::Error::Other(Error::Receive));
            }
            let b = self.rx_buf[0];
            Ok(b)
        } else {
            // If no RX is started, interrupt must be cleared
            Err(nb::Error::WouldBlock)
        }
    }
}

impl<T> crate::hal::uart::dma::DmaUartHal for Uarte<T>
where
    T: Instance,
{
    fn enable_interrupt(&self) {
        let uart = unsafe { &*T::ptr() };
        uart.inten
            .modify(|_, w| w.endrx().set_bit().endtx().set_bit().rxto().set_bit());
    }

    fn prepare_write(&self, tx_buffer: &[u8]) -> Result<(), Error> {
        if tx_buffer.len() > hal::target_constants::EASY_DMA_SIZE {
            return Err(Error::TxBufferTooLong);
        }

        // We can only DMA out of RAM.
        slice_in_ram_or(tx_buffer, Error::BufferNotInRAM)?;

        let uart = unsafe { &*T::ptr() };
        prepare_write(uart, tx_buffer);
        Ok(())
    }

    fn start_write(&self) {
        let uart = unsafe { &*T::ptr() };
        start_write(uart);
    }

    fn process_interrupts(&self) -> (bool, bool) {
        let uart = unsafe { &*T::ptr() };
        let tx_done =
            uart.events_endtx.read().bits() != 0 || uart.events_txstopped.read().bits() != 0;

        let rx_done = uart.events_endrx.read().bits() != 0;

        if uart.events_error.read().bits() != 0 {
            uart.events_error.reset();
        }

        if uart.events_txstarted.read().bits() != 0 {
            uart.events_txstarted.reset();
        }

        if uart.events_txstopped.read().bits() != 0 {
            uart.events_txstopped.reset();
        }

        if uart.events_endtx.read().bits() != 0 {
            uart.events_endtx.reset();
        }

        if uart.events_endrx.read().bits() != 0 {
            uart.events_endrx.reset();
        }

        (tx_done, rx_done)
    }

    fn finish_write(&self) -> Result<(), Error> {
        // Conservative compiler fence to prevent optimizations that do not
        // take in to account actions by DMA. The fence has been placed here,
        // after all possible DMA actions have completed.
        compiler_fence(SeqCst);

        let uart = unsafe { &*T::ptr() };
        if uart.events_txstopped.read().bits() != 0 {
            return Err(Error::Transmit);
        }

        stop_write(uart);
        Ok(())
    }

    /// Cancel a write operation
    fn cancel_write(&self) {
        let uart = unsafe { &*T::ptr() };
        cancel_write(uart);
    }

    fn prepare_read(&self, rx_buffer: &mut [u8]) -> Result<(), Error> {
        slice_in_ram_or(rx_buffer, Error::BufferNotInRAM)?;
        let uart = unsafe { &*T::ptr() };
        prepare_read(uart, rx_buffer)?;
        Ok(())
    }

    /// Start a read operation
    fn start_read(&self) {
        let uart = unsafe { &*T::ptr() };
        start_read(uart);
    }

    /// Complete a read operation.
    fn finish_read(&self) -> usize {
        let uart = unsafe { &*T::ptr() };
        with_critical_section(|_| {
            finalize_read(uart);

            let bytes_read = uart.rxd.amount.read().bits() as usize;

            bytes_read
        })
    }

    /// Cancel a read operation
    fn cancel_read(&self) {
        let uart = unsafe { &*T::ptr() };
        with_critical_section(|_| cancel_read(uart));
    }
}

// ---- Utilities copied from nrf-hal

/// Write via UARTE.
///
/// This method prepares DMA for writing all bytes in `tx_buffer`.
fn prepare_write(uarte: &uarte0::RegisterBlock, tx_buffer: &[u8]) {
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
}

/// Write via UARTE.
///
/// This method uses transmits all bytes in `tx_buffer`.
fn start_write(uarte: &uarte0::RegisterBlock) {
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
fn prepare_read(uarte: &uarte0::RegisterBlock, rx_buffer: &mut [u8]) -> Result<(), Error> {
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

    Ok(())
}

fn start_read(uarte: &uarte0::RegisterBlock) {
    // Start UARTE Receive transaction.
    uarte.tasks_startrx.write(|w| unsafe { w.bits(1) });
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

/// Does this slice reside entirely within RAM?
fn slice_in_ram(slice: &[u8]) -> bool {
    let ptr = slice.as_ptr() as usize;
    ptr >= hal::target_constants::SRAM_LOWER
        && (ptr + slice.len()) < hal::target_constants::SRAM_UPPER
}

/// Return an error if slice is not in RAM.
#[cfg(not(feature = "chip+nrf51"))]
pub(crate) fn slice_in_ram_or<T>(slice: &[u8], err: T) -> Result<(), T> {
    if slice_in_ram(slice) {
        Ok(())
    } else {
        Err(err)
    }
}
