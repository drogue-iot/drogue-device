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
        /*
        uart.inten.modify(|_, w| {
            w.endrx()
                .set_bit()
                .rxstarted()
                .set_bit()
                .rxto()
                .set_bit()
                .rxdrdy()
                .set_bit()
        });*/
        uart.inten.modify(|_, w| w.endtx().set_bit());

        Self { uart, pins }
    }
}

impl<T> crate::hal::uart::Uart for Uarte<T>
where
    T: Instance,
{
    fn write_start(&mut self, tx_buffer: &[u8]) -> Result<(), Error> {
        log::trace!("WRITE START");
        // We can only DMA out of RAM.
        slice_in_ram_or(tx_buffer, crate::hal::uart::Error::BufferNotInRAM)?;

        start_write(&*self.uart, tx_buffer);
        Ok(())
    }

    fn write_done(&mut self) -> bool {
        log::trace!("WRITE DONE?");
        self.uart.events_endtx.read().bits() != 0 || self.uart.events_txstopped.read().bits() != 0
    }

    fn write_finish(&mut self) -> Result<(), Error> {
        log::trace!("WRITE FINISH");
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
