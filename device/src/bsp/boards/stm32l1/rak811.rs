use crate::bsp::Board;
use crate::drivers::led::{ActiveHigh, Led};
use embassy::time::{block_for, Duration};
use embassy_lora::sx127x::*;
use embassy_stm32::dma::NoDma;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::pac;
use embassy_stm32::peripherals::{PA11, PA12, PA4, PB0, PB13, PB6, PB7, PH1, SPI1};
use embassy_stm32::spi;
use embassy_stm32::time::U32Ext;
use embedded_hal::digital::v2::OutputPin;
use rand::rngs::SmallRng;
use rand::SeedableRng;

pub type PinLedRed = Output<'static, PA12>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type Radio = Sx127xRadio<
    spi::Spi<'static, SPI1, NoDma, NoDma>,
    Output<'static, PB0>,
    Output<'static, PB13>,
    spi::Error,
    ExtiInput<'static, PA11>,
    RAKSwitch,
>;

pub type Rng = SmallRng;

pub struct Rak811 {
    pub led_red: LedRed,
    pub rng: Rng,
    pub radio: Radio,
}

impl Rak811 {
    pub fn config() -> embassy_stm32::Config {
        let mut config = embassy_stm32::Config::default();
        config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI);
        config
    }
}

impl Board for Rak811 {
    type Peripherals = embassy_stm32::Peripherals;
    fn new(p: Self::Peripherals) -> Self {
        unsafe {
            let rcc = pac::RCC;
            rcc.apb1enr().modify(|w| w.set_pwren(true));
            rcc.apb1rstr().modify(|w| w.set_pwrrst(true));
            rcc.apb1rstr().modify(|w| w.set_pwrrst(false));

            let pwr = pac::PWR;
            pwr.cr().modify(|w| w.set_vos(0b10));
        }

        // Generate seed value based on clock

        let mut seed: u32 = 0;
        let cp = cortex_m::peripheral::Peripherals::take().unwrap();
        let mut st = cp.SYST;
        st.set_reload(0x00FFFFFF);
        st.clear_current();
        st.enable_counter();
        let mut sample = 10;
        defmt::info!("Gathering entropy for random seed");
        for _ in 0..1000 {
            block_for(Duration::from_millis(sample as u64));
            sample = cortex_m::peripheral::SYST::get_current() & 0xF;
            seed += sample;
        }
        st.disable_counter();
        defmt::info!("Done");
        let rng = SmallRng::seed_from_u64(seed as u64);

        let crf1_pa = Output::new(p.PA4, Level::Low, Speed::Low);
        let crf2_hf = Output::new(p.PB7, Level::Low, Speed::Low);
        let crf3_rx = Output::new(p.PB6, Level::Low, Speed::Low);
        let xtal = Output::new(p.PH1, Level::High, Speed::Low);

        let rfs = RAKSwitch {
            crf1_pa,
            crf2_hf,
            crf3_rx,
            xtal,
        };

        // SPI for sx127x
        let spi = spi::Spi::new(
            p.SPI1,
            p.PA5,
            p.PA7,
            p.PA6,
            NoDma,
            NoDma,
            200_000.hz(),
            spi::Config::default(),
        );

        let cs = Output::new(p.PB0, Level::High, Speed::Low);
        let reset = Output::new(p.PB13, Level::High, Speed::Low);
        let _ = Input::new(p.PB2, Pull::None);

        let dio0 = Input::new(p.PA11, Pull::Up);
        let _dio1 = Input::new(p.PB1, Pull::Up);
        let _dio2 = Input::new(p.PA3, Pull::Up);
        let _dio3 = Input::new(p.PH0, Pull::Up);
        let _dio4 = Input::new(p.PC13, Pull::Up);

        let irq_pin = ExtiInput::new(dio0, p.EXTI11);
        let radio =
            Sx127xRadio::new(spi, cs, reset, irq_pin, rfs, &mut embassy::time::Delay).unwrap();

        Self {
            led_red: Led::new(Output::new(p.PA12, Level::Low, Speed::Low)),
            rng,
            radio,
        }
    }
}

pub struct RAKSwitch {
    crf1_pa: Output<'static, PA4>,
    crf2_hf: Output<'static, PB7>,
    crf3_rx: Output<'static, PB6>,
    xtal: Output<'static, PH1>,
}

impl RadioSwitch for RAKSwitch {
    fn set_rx(&mut self) {
        self.crf1_pa.set_low().unwrap();
        self.crf2_hf.set_low().unwrap();
        self.crf3_rx.set_high().unwrap();
    }

    fn set_tx(&mut self) {
        self.crf1_pa.set_high().unwrap();
        self.crf2_hf.set_low().unwrap();
        self.crf3_rx.set_low().unwrap();
    }
}
