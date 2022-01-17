use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_lora::sx127x::*;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{
    DMA1_CH2, DMA1_CH3, PA15, PA5, PB2, PB4, PB5, PB6, PC0, RNG, SPI1,
};
use embassy_stm32::spi;
use embassy_stm32::time::U32Ext;

pub type PinLedRed = Output<'static, PB5>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type PinLedGreen = Output<'static, PA5>;
pub type LedGreen = Led<PinLedGreen, ActiveHigh>;

pub type PinLedYellow = Output<'static, PB6>;
pub type LedYellow = Led<PinLedYellow, ActiveHigh>;

pub type PinUserButton = Input<'static, PB2>;
pub type UserButton = Button<ExtiInput<'static, PB2>>;

pub type Radio = Sx127xRadio<
    spi::Spi<'static, SPI1, DMA1_CH3, DMA1_CH2>,
    Output<'static, PA15>,
    Output<'static, PC0>,
    spi::Error,
    ExtiInput<'static, PB4>,
    DummySwitch,
>;

pub type Rng = embassy_stm32::rng::Rng<RNG>;

pub struct DummySwitch;
impl RadioSwitch for DummySwitch {
    fn set_rx(&mut self) {}
    fn set_tx(&mut self) {}
}

pub struct LoraDiscovery {
    pub led_red: LedRed,
    pub led_green: LedGreen,
    pub led_yellow: LedYellow,
    pub user_button: UserButton,
    pub rng: Rng,
    pub spi1: spi::Spi<'static, SPI1, DMA1_CH3, DMA1_CH2>,
    pub radio_cs: Output<'static, PA15>,
    pub radio_reset: Output<'static, PC0>,
    pub radio_ready: ExtiInput<'static, PB4>,
}

impl LoraDiscovery {
    pub fn config() -> embassy_stm32::Config {
        let mut config = embassy_stm32::Config::default();
        config.rcc.mux = embassy_stm32::rcc::ClockSrc::HSI16;
        config.rcc.enable_hsi48 = true;
        config.enable_debug_during_sleep = true;
        config
    }
}

impl Board for LoraDiscovery {
    type Peripherals = embassy_stm32::Peripherals;
    type BoardConfig = ();

    fn new(p: Self::Peripherals) -> Self {
        // SPI for sx127x
        let spi1 = spi::Spi::new(
            p.SPI1,
            p.PB3,
            p.PA7,
            p.PA6,
            p.DMA1_CH3,
            p.DMA1_CH2,
            200_000.hz(),
            spi::Config::default(),
        );

        let radio_cs = Output::new(p.PA15, Level::High, Speed::Low);
        let radio_reset = Output::new(p.PC0, Level::High, Speed::Low);
        let _ = Input::new(p.PB1, Pull::None);

        let ready = Input::new(p.PB4, Pull::Up);
        let radio_ready = ExtiInput::new(ready, p.EXTI4);

        // For RNG
        let rng = embassy_stm32::rng::Rng::new(p.RNG);

        Self {
            led_red: Led::new(Output::new(p.PB5, Level::Low, Speed::Low)),
            led_green: Led::new(Output::new(p.PA5, Level::Low, Speed::Low)),
            led_yellow: Led::new(Output::new(p.PB6, Level::Low, Speed::Low)),
            user_button: Button::new(ExtiInput::new(Input::new(p.PB2, Pull::Up), p.EXTI2)),
            rng,
            spi1,
            radio_cs,
            radio_reset,
            radio_ready,
        }
    }
}
