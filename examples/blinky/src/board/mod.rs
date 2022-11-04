pub struct Board;

#[cfg(feature = "board+b_l475e_iot01a")]
mod board {
    use embassy_stm32::{
        exti::ExtiInput,
        gpio::{Input, Level, Output, Pull, Speed},
        peripherals::{PB14, PC13},
    };
    impl crate::BlinkyBoard for super::Board {
        type Led = Output<'static, PB14>;
        type Button = ExtiInput<'static, PC13>;

        fn new() -> (Self::Led, Self::Button) {
            let p = embassy_stm32::init(Default::default());
            (
                Output::new(p.PB14, Level::Low, Speed::VeryHigh),
                ExtiInput::new(Input::new(p.PC13, Pull::Up), p.EXTI13),
            )
        }
    }
}

#[cfg(feature = "board+nrf52-dk")]
mod board {
    use embassy_nrf::{
        gpio::{Input, Level, Output, OutputDrive, Pull},
        peripherals::{P0_11, P0_17},
    };
    impl crate::BlinkyBoard for super::Board {
        type Led = Output<'static, P0_17>;
        type Button = Input<'static, P0_11>;

        fn new() -> (Self::Led, Self::Button) {
            let p = embassy_nrf::init(Default::default());
            (
                Output::new(p.P0_17, Level::Low, OutputDrive::Standard),
                Input::new(p.P0_11, Pull::Up),
            )
        }
    }
}

pub use board::*;
