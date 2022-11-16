#![no_std]
use {
    embassy_lora::{sx127x::*, LoraTimer},
    embassy_stm32::{
        exti::ExtiInput,
        gpio::{Input, Level, Output, Pull, Speed},
        pac,
        peripherals::{DMA1_CH2, DMA1_CH3, PA11, PA12, PA4, PB0, PB13, PB6, PB7, PH1, SPI1},
        spi,
        time::hz,
    },
    embassy_time::{block_for, Duration},
    rand::{rngs::SmallRng, SeedableRng},
};

pub use {lorawan::default_crypto::DefaultFactory as Crypto, lorawan_device::async_device::*};

pub type LedRed = Output<'static, PA12>;

pub type Radio = Sx127xRadio<
    spi::Spi<'static, SPI1, DMA1_CH3, DMA1_CH2>,
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
    pub spi1: spi::Spi<'static, SPI1, DMA1_CH3, DMA1_CH2>,
    pub radio_cs: Output<'static, PB0>,
    pub radio_reset: Output<'static, PB13>,
    pub radio_ready: ExtiInput<'static, PA11>,
    pub radio_switch: RAKSwitch,
}

impl Default for Rak811 {
    fn default() -> Self {
        let mut config = embassy_stm32::Config::default();
        config.rcc.mux = embassy_stm32::rcc::ClockSrc::HSI;
        config.enable_debug_during_sleep = true;
        Self::new(config)
    }
}

impl Rak811 {
    fn new(config: embassy_stm32::Config) -> Self {
        let p = embassy_stm32::init(config);
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
        for _ in 0..1000 {
            block_for(Duration::from_millis(sample as u64));
            sample = cortex_m::peripheral::SYST::get_current() & 0xF;
            seed += sample;
        }
        st.disable_counter();
        let rng = SmallRng::seed_from_u64(seed as u64);

        let crf1_pa = Output::new(p.PA4, Level::Low, Speed::Low);
        let crf2_hf = Output::new(p.PB7, Level::Low, Speed::Low);
        let crf3_rx = Output::new(p.PB6, Level::Low, Speed::Low);
        let xtal = Output::new(p.PH1, Level::High, Speed::Low);

        let radio_switch = RAKSwitch {
            crf1_pa,
            crf2_hf,
            crf3_rx,
            _xtal: xtal,
        };

        // SPI for sx127x
        let spi1 = spi::Spi::new(
            p.SPI1,
            p.PA5,
            p.PA7,
            p.PA6,
            p.DMA1_CH3,
            p.DMA1_CH2,
            hz(200_000),
            spi::Config::default(),
        );

        let radio_cs = Output::new(p.PB0, Level::High, Speed::Low);
        let radio_reset = Output::new(p.PB13, Level::High, Speed::Low);
        let _ = Input::new(p.PB2, Pull::None);

        let dio0 = Input::new(p.PA11, Pull::Up);
        let _dio1 = Input::new(p.PB1, Pull::Up);
        let _dio2 = Input::new(p.PA3, Pull::Up);
        let _dio3 = Input::new(p.PH0, Pull::Up);
        let _dio4 = Input::new(p.PC13, Pull::Up);

        let radio_ready = ExtiInput::new(dio0, p.EXTI11);

        Self {
            led_red: Output::new(p.PA12, Level::Low, Speed::Low),
            rng,
            spi1,
            radio_cs,
            radio_reset,
            radio_ready,
            radio_switch,
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

pub struct RAKSwitch {
    crf1_pa: Output<'static, PA4>,
    crf2_hf: Output<'static, PB7>,
    crf3_rx: Output<'static, PB6>,
    _xtal: Output<'static, PH1>,
}

impl RadioSwitch for RAKSwitch {
    fn set_rx(&mut self) {
        self.crf1_pa.set_low();
        self.crf2_hf.set_low();
        self.crf3_rx.set_high();
    }

    fn set_tx(&mut self) {
        self.crf1_pa.set_high();
        self.crf2_hf.set_low();
        self.crf3_rx.set_low();
    }
}
