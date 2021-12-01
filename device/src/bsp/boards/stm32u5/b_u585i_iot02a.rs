use crate::bsp::Board;
use crate::drivers::led::{ActiveHigh, ActiveLow, Led};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{PC13, PE13, PH6, PH7};

pub type PinLedBlue = Output<'static, PE13>;
pub type LedBlue = Led<PinLedBlue, ActiveHigh>;

pub type PinLedGreen = Output<'static, PH7>;
pub type LedGreen = Led<PinLedGreen, ActiveLow>;

pub type PinLedRed = Output<'static, PH6>;
pub type LedRed = Led<PinLedRed, ActiveLow>;

pub type PinUserButton = Input<'static, PC13>;
pub type UserButton = ExtiInput<'static, PC13>;

pub struct Iot02a {
    pub led_blue: LedBlue,
    pub led_green: LedGreen,
    pub led_red: LedRed,
    pub user_button: UserButton,
}

impl Board for Iot02a {
    type Peripherals = embassy_stm32::Peripherals;

    fn new(p: Self::Peripherals) -> Self {
        Self {
            led_blue: Led::new(Output::new(p.PE13, Level::High, Speed::Low)),
            led_green: Led::new(Output::new(p.PH7, Level::High, Speed::Low)),
            led_red: Led::new(Output::new(p.PH6, Level::High, Speed::Low)),
            user_button: ExtiInput::new(Input::new(p.PC13, Pull::Down), p.EXTI13),
        }
    }
}
