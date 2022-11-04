use crate::BlinkyBoard;

pub struct Board;

#[cfg(feature = "board+b_l475e_iot01a")]
mod board {
    use embassy_stm32::peripherals::PB14;
    impl BlinkyBoard for Board {
        type Led = PB14;
        fn new() -> (Self::Led, Self::Button) {
            let p = embassy_stm32::init(Default::default());
            (p.PB14, p.B2)
        }
    }
}

use board::*;
