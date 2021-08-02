#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use log::LevelFilter;
use panic_probe as _;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use drogue_device::{
    actors::{button::*, lora::*},
    drivers::led::*,
    drivers::lora::sx127x::*,
    traits::lora::*,
    *,
};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    interrupt,
    peripherals::{PA15, PA5, PB2, PB4, PB5, PB6, PB7, PC0, RNG, SPI1},
    rcc,
    rng::Random,
    spi,
    time::U32Ext,
    Peripherals,
};

use rand_core::RngCore;

mod app;

use app::*;

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));
static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Trace);

static mut RNG: Option<Random<RNG>> = None;
fn get_random_u32() -> u32 {
    unsafe {
        if let Some(rng) = &mut RNG {
            rng.reset();
            let result = rng.next_u32();
            result
        } else {
            panic!("No Rng exists!");
        }
    }
}

pub type Sx127x<'a> = Sx127xDriver<
    'a,
    ExtiInput<'a, PB4>,
    spi::Spi<'a, SPI1>,
    Output<'a, PA15>,
    Output<'a, PC0>,
    spi::Error,
>;

type Led1Pin = Output<'static, PB5>;
type Led2Pin = Output<'static, PA5>;
type Led3Pin = Output<'static, PB6>;
type Led4Pin = Output<'static, PB7>;

type MyApp = App<Address<'static, LoraActor<Sx127x<'static>>>, Led4Pin, Led2Pin, Led3Pin, Led1Pin>;

pub struct MyDevice {
    lora: ActorContext<'static, LoraActor<Sx127x<'static>>>,

    button: ActorContext<'static, Button<'static, ExtiInput<'static, PB2>, MyApp>>,
    app: ActorContext<'static, MyApp>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main(
    config = "embassy_stm32::Config::default().rcc(embassy_stm32::rcc::Config::default().clock_src(embassy_stm32::rcc::ClockSrc::HSI16))"
)]
async fn main(spawner: embassy::executor::Spawner, mut p: Peripherals) {
    rtt_init_print!();
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }

    log::set_max_level(log::LevelFilter::Trace);

    let mut rcc = rcc::Rcc::new(p.RCC);
    rcc.enable_debug_wfe(&mut p.DBGMCU, true);
    let _ = rcc.enable_hsi48(&mut p.SYSCFG, p.CRS);

    unsafe { RNG.replace(Random::new(p.RNG)) };

    let led1 = Led::new(Output::new(p.PB5, Level::Low, Speed::Low));
    let led2 = Led::new(Output::new(p.PA5, Level::Low, Speed::Low));
    let led3 = Led::new(Output::new(p.PB6, Level::Low, Speed::Low));
    let led4 = Led::new(Output::new(p.PB7, Level::Low, Speed::Low));

    let button = Input::new(p.PB2, Pull::Up);
    let pin = ExtiInput::new(button, p.EXTI2);

    // SPI for sx127x
    let spi = spi::Spi::new(
        p.SPI1,
        p.PB3,
        p.PA7,
        p.PA6,
        200_000.hz(),
        spi::Config::default(),
    );

    let cs = Output::new(p.PA15, Level::High, Speed::Low);
    let reset = Output::new(p.PC0, Level::High, Speed::Low);
    let _ = Input::new(p.PB1, Pull::None);

    let ready = Input::new(p.PB4, Pull::Up);
    let ready_pin = ExtiInput::new(ready, p.EXTI4);

    let lora = Sx127xDriver::new(ready_pin, spi, cs, reset, get_random_u32);

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .device_eui(&DEV_EUI.trim_end().into())
        .spreading_factor(SpreadingFactor::SF12)
        .app_eui(&APP_EUI.trim_end().into())
        .app_key(&APP_KEY.trim_end().into());

    log::info!("Configuring with config {:?}", config);

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::new(AppInitConfig {
            tx_led: led2,
            green_led: led1,
            init_led: led4,
            user_led: led3,
            lora: config,
        })),
        lora: ActorContext::new(LoraActor::new(lora)),
        button: ActorContext::new(Button::new(pin)),
    });

    DEVICE
        .mount(|device| async move {
            let lora = device.lora.mount((), spawner);
            let app = device.app.mount(AppConfig { lora }, spawner);
            device.button.mount(app, spawner);
        })
        .await;
}
