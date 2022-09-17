use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_lora::stm32wl::*;
use embassy_stm32::dma::NoDma;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::flash::Flash;
use embassy_stm32::gpio::Pin;
use embassy_stm32::gpio::{AnyPin, Input, Level, Output, Pull, Speed};
use embassy_stm32::interrupt;
use embassy_stm32::pac;
use embassy_stm32::peripherals::{PA0, PA1, PB11, PB15, PB9, PC6, RNG};
use embassy_stm32::subghz::*;

pub type PinLedBlue = Output<'static, PB15>;
pub type LedBlue = Led<PinLedBlue, ActiveHigh>;

pub type PinLedGreen = Output<'static, PB9>;
pub type LedGreen = Led<PinLedGreen, ActiveHigh>;

pub type PinLedRed = Output<'static, PB11>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type PinUserButtonB1 = Input<'static, PA0>;
pub type UserButtonB1 = Button<ExtiInput<'static, PA0>>;

pub type PinUserButtonB2 = Input<'static, PA1>;
pub type UserButtonB2 = Button<ExtiInput<'static, PA1>>;

pub type PinUserButtonB3 = Input<'static, PC6>;
pub type UserButtonB3 = Button<ExtiInput<'static, PC6>>;

pub type Radio = SubGhzRadio<'static, RadioSwitch<'static>>;
pub type Rng = embassy_stm32::rng::Rng<'static, RNG>;

pub struct NucleoWl55 {
    pub blue_led: LedBlue,
    pub green_led: LedGreen,
    pub red_led: LedRed,
    pub user_button_b1: UserButtonB1,
    pub user_button_b2: UserButtonB2,
    pub user_button_b3: UserButtonB3,
    pub rng: Rng,
    pub radio: Radio,
    pub flash: Flash<'static>,
}

impl NucleoWl55 {
    pub fn config(enable_debug: bool) -> embassy_stm32::Config {
        let mut config = embassy_stm32::Config::default();
        config.rcc.mux = embassy_stm32::rcc::ClockSrc::HSI16;
        config.rcc.enable_lsi = true;
        config.enable_debug_during_sleep = enable_debug;
        config
    }
}

impl Board for NucleoWl55 {
    type Peripherals = embassy_stm32::Peripherals;
    type BoardConfig = ();
    fn new(p: Self::Peripherals) -> Self {
        unsafe {
            pac::RCC.ccipr().modify(|w| {
                w.set_rngsel(0b01);
            });
        }
        let flash = Flash::unlock(p.FLASH);

        let blue_led = Led::new(Output::new(p.PB15, Level::Low, Speed::Low));
        let green_led = Led::new(Output::new(p.PB9, Level::Low, Speed::Low));
        let red_led = Led::new(Output::new(p.PB11, Level::Low, Speed::Low));

        let button_b1 = Input::new(p.PA0, Pull::Up);
        let user_button_b1 = Button::new(ExtiInput::new(button_b1, p.EXTI0));
        let button_b2 = Input::new(p.PA1, Pull::Up);
        let user_button_b2 = Button::new(ExtiInput::new(button_b2, p.EXTI1));
        let button_b3 = Input::new(p.PC6, Pull::Up);
        let user_button_b3 = Button::new(ExtiInput::new(button_b3, p.EXTI6));

        let ctrl1 = Output::new(p.PC3.degrade(), Level::High, Speed::High);
        let ctrl2 = Output::new(p.PC4.degrade(), Level::High, Speed::High);
        let ctrl3 = Output::new(p.PC5.degrade(), Level::High, Speed::High);
        let rfs = RadioSwitch::new(ctrl1, ctrl2, ctrl3);

        let radio = SubGhz::new(p.SUBGHZSPI, NoDma, NoDma);
        let irq = interrupt::take!(SUBGHZ_RADIO);
        let mut radio_config = SubGhzRadioConfig::default();
        radio_config.calibrate_image = CalibrateImage::ISM_863_870;
        let radio = SubGhzRadio::new(radio, rfs, irq, radio_config).unwrap();
        let rng = Rng::new(p.RNG);
        Self {
            blue_led,
            green_led,
            red_led,
            user_button_b1,
            user_button_b2,
            user_button_b3,
            rng,
            radio,
            flash,
        }
    }
}

pub struct RadioSwitch<'a> {
    ctrl1: Output<'a, AnyPin>,
    ctrl2: Output<'a, AnyPin>,
    ctrl3: Output<'a, AnyPin>,
}

impl<'a> RadioSwitch<'a> {
    fn new(
        ctrl1: Output<'a, AnyPin>,
        ctrl2: Output<'a, AnyPin>,
        ctrl3: Output<'a, AnyPin>,
    ) -> Self {
        Self {
            ctrl1,
            ctrl2,
            ctrl3,
        }
    }
}

impl<'a> embassy_lora::stm32wl::RadioSwitch for RadioSwitch<'a> {
    fn set_rx(&mut self) {
        self.ctrl1.set_high();
        self.ctrl2.set_low();
        self.ctrl3.set_high();
    }

    fn set_tx(&mut self) {
        self.ctrl1.set_high();
        self.ctrl2.set_high();
        self.ctrl3.set_high();
    }
}
