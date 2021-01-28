//! Timers for nRF series
#[cfg(feature = "nrf52833")]
use nrf52833_hal as hal;

use embedded_hal::prelude::*;

use crate::domain::time::{
    duration::{Duration, Milliseconds},
    fixed_point::FixedPoint,
    rate::*,
};

const TIMER_FREQUENCY: Hertz = Hertz(1_000_000);

pub struct Timer<T>
where
    T: hal::timer::Instance,
{
    timer: hal::timer::Timer<T>,
}

impl<T> Timer<T>
where
    T: hal::timer::Instance,
{
    pub fn new(timer: T) -> Self {
        Self {
            // NOTE: The HAL hardcodes timer frequency to 1MHz
            timer: hal::timer::Timer::new(timer),
        }
    }

    fn free(self) -> T {
        self.timer.free()
    }
}

impl<T> crate::hal::timer::Timer for Timer<T>
where
    T: hal::timer::Instance,
{
    fn start(&mut self, duration: Milliseconds) {
        let clock_rate: Millihertz<u32> = TIMER_FREQUENCY.into();
        let deadline: Millihertz<u32> = duration.to_rate::<Millihertz>().unwrap();

        let cycles = *clock_rate.integer() / *deadline.integer() as u32;
        self.timer.enable_interrupt();
        self.timer.start(cycles);
    }

    fn clear_update_interrupt_flag(&mut self) {
        self.timer.disable_interrupt();
    }
}
