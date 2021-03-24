use stm32l0xx_hal::{
    pac::{RCC, TIM2, TIM21, TIM22, TIM3, TIM6},
    rcc::{Clocks, Rcc},
};

use crate::domain::time::{
    duration::{Duration, Milliseconds},
    fixed_point::FixedPoint,
    rate::{Hertz, Millihertz},
};
use core::sync::atomic::{compiler_fence, Ordering::SeqCst};

/// Hardware timers
pub struct HardwareTimer<T> {
    clocks: Clocks,
    tim: T,
}

macro_rules! timers {
    ($($TIM:ident: ($tim:ident, $timXen:ident, $timXrst:ident, $apb:ident, $apbenr:ident, $apbrstr:ident, $timclk:ident, $mms:ty),)+) => {
        $(
            impl HardwareTimer<$TIM> {
                pub fn $tim(tim: $TIM, rcc: &mut Rcc) -> Self
                {
                    unsafe {
                        (&(*RCC::ptr()).$apbenr).modify(|_,w| w.$timXen().set_bit());
                        (&(*RCC::ptr()).$apbrstr).modify(|_,w| w.$timXrst().set_bit());
                        (&(*RCC::ptr()).$apbrstr).modify(|_,w| w.$timXrst().clear_bit());
                    }
                    Self {
                        clocks: rcc.clocks,
                        tim,
                    }
                }
            }

            impl $crate::hal::timer::Timer for HardwareTimer<$TIM> {
                fn start(&mut self, duration: Milliseconds) {
                    // pause
                    self.tim.cr1.modify(|_, w| w.cen().clear_bit());

                    // reset counter
                    self.tim.cnt.reset();

                    let deadline: Millihertz<u32> = duration.to_rate::<Millihertz>().unwrap();
                    let clock_rate: Millihertz<u64> = Hertz(self.clocks.$timclk().0).into();
                    let ticks = *clock_rate.integer() / *deadline.integer() as u64;
                    let psc = ((ticks - 1) / (1 << 16));

                    self.tim.psc.write(|w| w.psc().bits(psc as u16));

                    let arr = ((ticks / (psc + 1)) & 0xFFFF) as u16;
                    // This is only unsafe for some timers, so we need this to
                    // suppress the warnings.
                    self.tim.dier.write(|w| w.uie().clear_bit());
                    #[allow(unused_unsafe)]
                    self.tim.arr.write(|w|
                        unsafe {
                            w.arr().bits(arr as u16)
                        }
                    );

                    // Load prescaler value and reset its counter.
                    // Setting URS makes sure no interrupt is generated.
                    self.tim.cr1.modify(|_, w| w.urs().set_bit());
                    self.tim.egr.write(|w| w.ug().set_bit());
                    self.clear_update_interrupt_flag();

                    self.tim.dier.write(|w| w.uie().set_bit());

                    self.tim.cr1.modify(|_, w| w.cen().set_bit());
                }

                fn clear_update_interrupt_flag(&mut self) {
                    self.tim.sr.write(|w| w.uif().clear_bit());
                    self.tim.cr1.modify(|_, w| w.cen().clear_bit());
                }
            }
        )+
    }
}

timers! {
    TIM2: (tim2, tim2en, tim2rst, APB1ENR, apb1enr, apb1rstr, apb1_tim_clk,
        tim2::cr2::MMS_A),
    TIM3: (tim3, tim3en, tim3rst, APB1ENR, apb1enr, apb1rstr, apb1_tim_clk,
        tim2::cr2::MMS_A),
    TIM6: (tim6, tim6en, tim6rst, APB1ENR, apb1enr, apb1rstr, apb1_tim_clk,
        tim6::cr2::MMS_A),
    TIM21: (tim21, tim21en, tim21rst, APB2ENR, apb2enr, apb2rstr, apb2_tim_clk,
        tim21::cr2::MMS_A),
    TIM22: (tim22, tim22en, tim22rst, APB2ENR, apb2enr, apb2rstr, apb2_tim_clk,
        tim22::cr2::MMS_A),
}
