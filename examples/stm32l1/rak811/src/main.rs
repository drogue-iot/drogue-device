#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use embedded_hal::digital::v2::OutputPin;
use panic_probe as _;

use drogue_device::{
    actors::lora::*,
    drivers::led::*,
    drivers::lora::{sx127x::*, *},
    traits::lora::*,
    *,
};
use embassy::time::Duration;
use embassy_stm32::{
    dbgmcu::Dbgmcu,
    dma::NoDma,
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    pac,
    peripherals::{PA11, PA12, PA4, PB0, PB13, PB6, PB7, PH1, SPI1},
    spi,
    time::U32Ext,
    Peripherals,
};

mod app;
use app::*;

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));

use embassy::time::Timer;
use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
static mut RNG: Option<SmallRng> = None;

fn get_random_u32() -> u32 {
    unsafe {
        if let Some(rng) = &mut RNG {
            let result = rng.next_u32();
            result
        } else {
            panic!("No Rng exists!");
        }
    }
}

pub type Sx127x<'a> = LoraDevice<
    'a,
    Sx127xRadio<
        'a,
        spi::Spi<'a, SPI1, NoDma, NoDma>,
        Output<'a, PB0>,
        Output<'a, PB13>,
        spi::Error,
        ExtiInput<'a, PA11>,
        RAKSwitch,
    >,
>;

type Led1 = Led<Output<'static, PA12>>;

pub struct MyDevice {
    lora: ActorContext<'static, LoraActor<Sx127x<'static>>>,
    app: ActorContext<'static, App<Address<'static, LoraActor<Sx127x<'static>>>, Led1>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn config() -> embassy_stm32::Config {
    let mut config = embassy_stm32::Config::default();
    config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI);
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();

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
        Timer::after(Duration::from_millis(sample as u64)).await;
        sample = cortex_m::peripheral::SYST::get_current() & 0xF;
        seed += sample;
    }
    st.disable_counter();
    defmt::info!("Done");
    unsafe {
        RNG.replace(SmallRng::seed_from_u64(seed as u64));
    }

    let led1 = Led::new(Output::new(p.PA12, Level::Low, Speed::Low));

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
        1_000_000.hz(),
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

    static mut RADIO_TX_BUF: [u8; 255] = [0; 255];
    static mut RADIO_RX_BUF: [u8; 255] = [0; 255];
    let lora = unsafe {
        LoraDevice::new(
            Sx127xRadio::new(spi, cs, reset, &mut RADIO_RX_BUF, rfs),
            Sx127xRadioIrq::new(irq_pin),
            get_random_u32,
            &mut RADIO_TX_BUF,
        )
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .device_eui(&DEV_EUI.trim_end().into())
        .spreading_factor(SpreadingFactor::SF9)
        .app_eui(&APP_EUI.trim_end().into())
        .app_key(&APP_KEY.trim_end().into());

    defmt::info!("Configuring with config {:?}", config);

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::new(config, led1, Duration::from_secs(60))),
        lora: ActorContext::new(LoraActor::new(lora)),
    });

    DEVICE
        .mount(|device| async move {
            let lora = device.lora.mount((), spawner);
            device.app.mount(lora, spawner);
        })
        .await;
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
