use crate::bsp::Board;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{PB0, PB14, PC13, PE1};

pub type PinLedRed = Output<'static, PB14>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type PinLedGreen = Output<'static, PB0>;
pub type LedGreen = Led<PinLedGreen, ActiveHigh>;

pub type PinLedYellow = Output<'static, PE1>;
pub type LedYellow = Led<PinLedYellow, ActiveHigh>;

pub type PinUserButton = Input<'static, PC13>;
pub type UserButton = ExtiInput<'static, PC13>;

pub struct NucleoH743 {
    pub led_red: LedRed,
    pub led_green: LedGreen,
    pub led_yellow: LedYellow,
    pub user_button: UserButton,
}

impl Board for NucleoH743 {
    type Peripherals = embassy_stm32::Peripherals;

    fn new(p: Self::Peripherals) -> Self {
        Self {
            led_red: Led::new(Output::new(p.PB14, Level::High, Speed::Low)),
            led_green: Led::new(Output::new(p.PB0, Level::High, Speed::Low)),
            led_yellow: Led::new(Output::new(p.PE1, Level::High, Speed::Low)),
            user_button: ExtiInput::new(Input::new(p.PC13, Pull::Down), p.EXTI13),
        }
    }
}
