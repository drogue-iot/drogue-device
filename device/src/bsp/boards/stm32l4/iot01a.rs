use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, ActiveLow, Led};
pub use crate::drivers::wifi::eswifi::AdapterMode;
use crate::drivers::wifi::eswifi::EsWifi as EsWifiController;
use embassy_stm32::dma::NoDma;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::i2c;
use embassy_stm32::interrupt;
use embassy_stm32::peripherals::{
    DMA1_CH4, DMA1_CH5, I2C2, PA5, PB13, PB14, PC13, PD15, PE0, PE1, PE8, RNG, SPI3,
};
use embassy_stm32::rcc::{AHBPrescaler, ClockSrc, PLLClkDiv, PLLMul, PLLSource, PLLSrcDiv};
use embassy_stm32::rng;
use embassy_stm32::spi;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

pub type PinLedBlue = Output<'static, PA5>;
pub type LedBlue = Led<PinLedBlue, ActiveHigh>;

pub type PinLedGreen = Output<'static, PB14>;
pub type LedGreen = Led<PinLedGreen, ActiveLow>;

pub type PinUserButton = Input<'static, PC13>;
pub type UserButton = Button<ExtiInput<'static, PC13>>;

pub type I2c2 = i2c::I2c<'static, I2C2, DMA1_CH4, DMA1_CH5>;

pub type Hts221Ready = ExtiInput<'static, PD15>;

pub type Rng = rng::Rng<'static, RNG>;

pub type WifiWake = Output<'static, PB13>;
pub type WifiReset = Output<'static, PE8>;
pub type WifiCs = Output<'static, PE0>;
pub type WifiReady = ExtiInput<'static, PE1>;
type SPI = spi::Spi<'static, SPI3, NoDma, NoDma>; // DMA2_CH2, DMA2_CH1>;
type SpiError = spi::Error;

pub type EsWifi = EsWifiController<SPI, WifiCs, WifiReset, WifiWake, WifiReady, SpiError>;

pub struct Iot01a {
    pub led_blue: LedBlue,
    pub led_green: LedGreen,
    pub user_button: UserButton,
    pub i2c2: I2c2,
    pub hts221_ready: Hts221Ready,
    pub rng: Rng,
    pub wifi: EsWifi,
}

impl Iot01a {
    pub fn config(enable_debug: bool) -> Config {
        let mut config = Config::default();
        config.rcc.mux = ClockSrc::PLL(
            PLLSource::HSI16,
            PLLClkDiv::Div2,
            PLLSrcDiv::Div1,
            PLLMul::Mul10,
            Some(PLLClkDiv::Div2),
        );
        config.rcc.ahb_pre = AHBPrescaler::Div8;
        config.enable_debug_during_sleep = enable_debug;
        config
    }
}

impl Board for Iot01a {
    type Peripherals = embassy_stm32::Peripherals;
    type BoardConfig = ();

    fn new(p: Self::Peripherals) -> Self {
        let i2c2_irq = interrupt::take!(I2C2_EV);
        let i2c2 = i2c::I2c::new(
            p.I2C2,
            p.PB10,
            p.PB11,
            i2c2_irq,
            p.DMA1_CH4,
            p.DMA1_CH5,
            Hertz(100_000),
        );

        let hts221_ready_pin = Input::new(p.PD15, Pull::Down);
        let hts221_ready = ExtiInput::new(hts221_ready_pin, p.EXTI15);

        let rng = rng::Rng::new(p.RNG);

        let spi = spi::Spi::new(
            p.SPI3,
            p.PC10,
            p.PC12,
            p.PC11,
            NoDma,
            NoDma,
            Hertz(100_000),
            spi::Config::default(),
        );

        let _boot = Output::new(p.PB12, Level::Low, Speed::VeryHigh);
        let wake = Output::new(p.PB13, Level::Low, Speed::VeryHigh);
        let reset = Output::new(p.PE8, Level::Low, Speed::VeryHigh);
        let cs = Output::new(p.PE0, Level::High, Speed::VeryHigh);
        let ready = Input::new(p.PE1, Pull::Up);
        let ready = ExtiInput::new(ready, p.EXTI1);

        let wifi = EsWifiController::new(spi, cs, reset, wake, ready);
        /*
        match wifi.start().await {
            Ok(()) => info!("Started..."),
            Err(err) => info!("Error... {}", err),
        }
        */

        Self {
            led_blue: Led::new(Output::new(p.PA5, Level::High, Speed::Low)),
            led_green: Led::new(Output::new(p.PB14, Level::High, Speed::Low)),
            user_button: Button::new(ExtiInput::new(Input::new(p.PC13, Pull::Up), p.EXTI13)),
            i2c2,
            hts221_ready,
            rng,
            wifi,
        }
    }
}
