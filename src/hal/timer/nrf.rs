//! Timers for nRF series
#[cfg(feature = "nrf52833")]
use nrf52833_hal as hal;

use crate::domain::time::{
    duration::{Duration, Milliseconds},
    fixed_point::FixedPoint,
    rate::*,
};

use embedded_hal::timer::CountDown;
pub use hal::timer::*;

impl<T> crate::hal::timer::Timer for Timer<T, OneShot>
where
    T: Instance,
{
    fn start(&mut self, duration: Milliseconds) {
        let clock_rate: Millihertz<u32> = Hertz(Self::TICKS_PER_SECOND).into();
        let deadline: Millihertz<u32> = duration.to_rate::<Millihertz>().unwrap();

        let cycles = *clock_rate.integer() / *deadline.integer() as u32;
        self.enable_interrupt();
        CountDown::start(self, cycles);
    }

    fn clear_update_interrupt_flag(&mut self) {
        self.disable_interrupt();
    }
}
