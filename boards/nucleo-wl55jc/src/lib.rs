#![no_std]
#[allow(unused_imports)]
use embassy_lora::stm32wl::*;
use {
    embassy_lora::LoraTimer,
    embassy_stm32::{
        dma::NoDma,
        exti::ExtiInput,
        flash::Flash,
        gpio::{AnyPin, Input, Level, Output, Pin, Pull, Speed},
        interrupt, pac,
        peripherals::{PA0, PA1, PB11, PB15, PB9, PC6, RNG},
        subghz::*,
    },
};
pub use {lorawan::default_crypto::DefaultFactory as Crypto, lorawan_device::async_device::*};

pub type LedBlue = Output<'static, PB15>;
pub type LedGreen = Output<'static, PB9>;
pub type LedRed = Output<'static, PB11>;

pub type UserButtonB1 = ExtiInput<'static, PA0>;
pub type UserButtonB2 = ExtiInput<'static, PA1>;
pub type UserButtonB3 = ExtiInput<'static, PC6>;

pub type Radio = SubGhzRadio<'static, RadioSwitch<'static>>;
pub type Rng = embassy_stm32::rng::Rng<'static, RNG>;

pub type LorawanDevice = Device<Radio, Crypto, LoraTimer, Rng>;

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

impl Default for NucleoWl55 {
    fn default() -> Self {
        let mut config = embassy_stm32::Config::default();
        config.rcc.mux = embassy_stm32::rcc::ClockSrc::HSI16;
        config.rcc.enable_lsi = true;
        config.enable_debug_during_sleep = true;
        Self::new(config)
    }
}

impl NucleoWl55 {
    fn new(config: embassy_stm32::Config) -> Self {
        let p = embassy_stm32::init(config);
        unsafe {
            pac::RCC.ccipr().modify(|w| {
                w.set_rngsel(0b01);
            });
        }
        let flash = Flash::new(p.FLASH);

        let blue_led = Output::new(p.PB15, Level::Low, Speed::Low);
        let green_led = Output::new(p.PB9, Level::Low, Speed::Low);
        let red_led = Output::new(p.PB11, Level::Low, Speed::Low);

        let button_b1 = Input::new(p.PA0, Pull::Up);
        let user_button_b1 = ExtiInput::new(button_b1, p.EXTI0);
        let button_b2 = Input::new(p.PA1, Pull::Up);
        let user_button_b2 = ExtiInput::new(button_b2, p.EXTI1);
        let button_b3 = Input::new(p.PC6, Pull::Up);
        let user_button_b3 = ExtiInput::new(button_b3, p.EXTI6);

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

    pub fn lorawan(
        region: region::Configuration,
        radio: Radio,
        rng: Rng,
    ) -> Device<Radio, Crypto, LoraTimer, Rng> {
        Device::new(region, radio, LoraTimer::new(), rng)
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
