#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::{button::*, lora::*},
    drivers::led::*,
    drivers::lora::*,
    traits::lora::*,
    *,
};
use drogue_device_macros::drogue_config;
use embassy_lora::sx127x::*;
use embassy_stm32::{
    dbgmcu::Dbgmcu,
    dma::NoDma,
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    peripherals::{PA15, PA5, PB2, PB4, PB5, PB6, PB7, PC0, RNG, SPI1},
    rcc,
    rng::Rng,
    spi,
    time::U32Ext,
    Peripherals,
};

mod app;

use app::*;

const DEV_EUI: &str = drogue_config!("dev-eui");
const APP_EUI: &str = drogue_config!("app-eui");
const APP_KEY: &str = drogue_config!("app-key");

pub type Sx127x<'a> = LoraDevice<
    'a,
    Sx127xRadio<
        spi::Spi<'a, SPI1, NoDma, NoDma>,
        Output<'a, PA15>,
        Output<'a, PC0>,
        spi::Error,
        ExtiInput<'a, PB4>,
        DummySwitch,
    >,
    Rng<RNG>,
>;

type Led1 = Led<Output<'static, PB5>>;
type Led2 = Led<Output<'static, PA5>>;
type Led3 = Led<Output<'static, PB6>>;
type Led4 = Led<Output<'static, PB7>>;

type MyApp = App<Address<'static, LoraActor<Sx127x<'static>>>, Led4, Led2, Led3, Led1>;

pub struct MyDevice {
    lora: ActorContext<'static, LoraActor<Sx127x<'static>>>,

    button: ActorContext<'static, Button<ExtiInput<'static, PB2>, ButtonEventDispatcher<MyApp>>>,
    app: ActorContext<'static, MyApp>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn config() -> embassy_stm32::Config {
    let mut config = embassy_stm32::Config::default();
    config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI16);
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, mut p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let mut rcc = rcc::Rcc::new(p.RCC);
    let _ = rcc.enable_hsi48(&mut p.SYSCFG, p.CRS);

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
    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    static mut RADIO_BUF: [u8; 256] = [0; 256];
    let lora = unsafe { LoraDevice::new(&config, radio, Rng::new(p.RNG), &mut RADIO_BUF).unwrap() };

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::new(AppInitConfig {
            tx_led: led2,
            green_led: led1,
            init_led: led4,
            user_led: led3,
            join_mode,
        })),
        lora: ActorContext::new(LoraActor::new(lora)),
        button: ActorContext::new(Button::new(pin)),
    });

    DEVICE
        .mount(|device| async move {
            let lora = device.lora.mount((), spawner);
            let app = device.app.mount(AppConfig { lora }, spawner);
            device.button.mount(app.into(), spawner);
        })
        .await;
}

pub struct DummySwitch;
impl RadioSwitch for DummySwitch {
    fn set_rx(&mut self) {}
    fn set_tx(&mut self) {}
}
