use crate::bsp::Board;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    gpiote::PortInput,
    peripherals::{P1_02, P1_09},
};

pub type PinLedRed = Output<'static, P1_09>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type PinUserButton = Input<'static, P1_02>;
pub type UserButton = PortInput<'static, P1_02>;

pub struct AdafruitFeatherSense {
    pub led_red: LedRed,
    pub user_button: UserButton,
}

impl Board for AdafruitFeatherSense {
    type Peripherals = embassy_nrf::Peripherals;

    fn new(p: Self::Peripherals) -> Self {
        Self {
            led_red: Led::new(Output::new(p.P1_09, Level::High, OutputDrive::Standard)),
            user_button: PortInput::new(Input::new(p.P1_02, Pull::Down)),
        }
    }
}
