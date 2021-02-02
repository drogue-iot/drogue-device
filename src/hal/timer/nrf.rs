//! Timers for nRF series
#[cfg(feature = "nrf52833")]
use nrf52833_hal as hal;

use crate::domain::time::{
    duration::{Duration, Milliseconds},
    fixed_point::FixedPoint,
    rate::*,
};

use embedded_hal::timer::CountDown;
use hal::timer::{Instance, OneShot, Timer as NrfTimer};

pub struct Timer<T>
where
    T: Instance,
{
    timer: NrfTimer<T, OneShot>,
}

impl<T> Timer<T>
where
    T: Instance,
{
    pub fn new(timer: T) -> Self {
        let mut timer = NrfTimer::new(timer);
        timer.enable_interrupt();
        Self { timer }
    }
}

impl<T> crate::hal::timer::Timer for Timer<T>
where
    T: Instance,
{
    fn start(&mut self, duration: Milliseconds) {
        let clock_rate: Millihertz<u32> = Hertz(NrfTimer::<T, OneShot>::TICKS_PER_SECOND).into();
        let deadline: Millihertz<u32> = duration.to_rate::<Millihertz>().unwrap();

        let cycles = *clock_rate.integer() / *deadline.integer() as u32;
        // log::info!("Delaying for {} cycles", cycles);
        CountDown::start(&mut self.timer, cycles);
    }

    fn clear_update_interrupt_flag(&mut self) {
        self.timer.task_stop().write(|w| unsafe { w.bits(1) });
        self.timer.event_compare_cc0().write(|w| w);
    }
}
