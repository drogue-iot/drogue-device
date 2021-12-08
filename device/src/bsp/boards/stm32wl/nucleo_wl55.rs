use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_lora::stm32wl::*;
use embassy_stm32::dma::NoDma;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::Pin;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::interrupt;
use embassy_stm32::pac;
use embassy_stm32::peripherals::{PA0, PB11, PB15, PB9, RNG};
use embassy_stm32::rcc::Rcc;
use embassy_stm32::subghz::*;

pub type PinLedBlue = Output<'static, PB15>;
pub type LedBlue = Led<PinLedBlue, ActiveHigh>;

pub type PinLedGreen = Output<'static, PB9>;
pub type LedGreen = Led<PinLedGreen, ActiveHigh>;

pub type PinLedYellow = Output<'static, PB11>;
pub type LedYellow = Led<PinLedYellow, ActiveHigh>;

pub type PinUserButton = Input<'static, PA0>;
pub type UserButton = Button<ExtiInput<'static, PA0>>;

pub type Radio = SubGhzRadio<'static>;
pub type Rng = embassy_stm32::rng::Rng<RNG>;

pub struct NucleoWl55 {
    pub led_blue: LedBlue,
    pub led_green: LedGreen,
    pub led_yellow: LedYellow,
    pub user_button: UserButton,
    pub rng: Rng,
    pub rcc: Rcc<'static>,
    pub radio: Radio,
}

impl NucleoWl55 {
    pub fn config() -> embassy_stm32::Config {
        let mut config = embassy_stm32::Config::default();
        config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI16);
        config
    }
}

impl Board for NucleoWl55 {
    type Peripherals = embassy_stm32::Peripherals;
    fn new(p: Self::Peripherals) -> Self {
        let mut rcc = Rcc::new(p.RCC);
        unsafe {
            rcc.enable_lsi();
            pac::RCC.ccipr().modify(|w| {
                w.set_rngsel(0b01);
            });
        }

        let led_blue = Led::new(Output::new(p.PB15, Level::Low, Speed::Low));
        let led_green = Led::new(Output::new(p.PB9, Level::Low, Speed::Low));
        let led_yellow = Led::new(Output::new(p.PB11, Level::Low, Speed::Low));

        let button = Input::new(p.PA0, Pull::Up);
        let user_button = Button::new(ExtiInput::new(button, p.EXTI0));

        let ctrl1 = Output::new(p.PC3.degrade(), Level::High, Speed::High);
        let ctrl2 = Output::new(p.PC4.degrade(), Level::High, Speed::High);
        let ctrl3 = Output::new(p.PC5.degrade(), Level::High, Speed::High);
        let rfs = RadioSwitch::new(ctrl1, ctrl2, ctrl3);

        let radio = SubGhz::new(p.SUBGHZSPI, p.PA5, p.PA7, p.PA6, NoDma, NoDma);

        static mut RADIO_STATE: SubGhzState<'static> = SubGhzState::new();
        let irq = interrupt::take!(SUBGHZ_RADIO);
        let radio = unsafe { SubGhzRadio::new(&mut RADIO_STATE, radio, rfs, irq) };
        let rng = Rng::new(p.RNG);
        Self {
            led_blue,
            led_green,
            led_yellow,
            user_button,
            rcc,
            rng,
            radio,
        }
    }
}
