use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, Led};
use embassy_lora::sx127x::*;
use embassy_stm32::dma::NoDma;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::{PA15, PA5, PB2, PB4, PB5, PB6, PC0, RNG, SPI1};
use embassy_stm32::rcc::Rcc;
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
    spi::Spi<'static, SPI1, NoDma, NoDma>,
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
    pub rcc: Rcc<'static>,
    pub radio: Radio,
}

impl LoraDiscovery {
    pub fn config() -> embassy_stm32::Config {
        let mut config = embassy_stm32::Config::default();
        config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI16);
        config
    }
}

impl Board for LoraDiscovery {
    type Peripherals = embassy_stm32::Peripherals;
    fn new(mut p: Self::Peripherals) -> Self {
        // SPI for sx127x
        let spi = spi::Spi::new(
            p.SPI1,
            p.PB3,
            p.PA7,
            p.PA6,
            NoDma,
            NoDma,
            200_000.hz(),
            spi::Config::default(),
        );

        let cs = Output::new(p.PA15, Level::High, Speed::Low);
        let reset = Output::new(p.PC0, Level::High, Speed::Low);
        let _ = Input::new(p.PB1, Pull::None);

        let ready = Input::new(p.PB4, Pull::Up);
        let ready_pin = ExtiInput::new(ready, p.EXTI4);

        let radio = Sx127xRadio::new(
            spi,
            cs,
            reset,
            ready_pin,
            DummySwitch,
            &mut embassy::time::Delay,
        )
        .unwrap();

        // For RNG
        let mut rcc = Rcc::new(p.RCC);
        let _ = rcc.enable_hsi48(&mut p.SYSCFG, p.CRS);
        let rng = embassy_stm32::rng::Rng::new(p.RNG);

        Self {
            led_red: Led::new(Output::new(p.PB5, Level::Low, Speed::Low)),
            led_green: Led::new(Output::new(p.PA5, Level::Low, Speed::Low)),
            led_yellow: Led::new(Output::new(p.PB6, Level::Low, Speed::Low)),
            user_button: Button::new(ExtiInput::new(Input::new(p.PB2, Pull::Up), p.EXTI2)),
            rcc,
            rng,
            radio,
        }
    }
}
