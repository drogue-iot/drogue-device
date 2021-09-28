#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;
//use panic_halt as _;

use drogue_device::{
    actors::{button::*, lora::*},
    drivers::led::*,
    drivers::lora::{stm32wl::*, *},
    traits::lora::{JoinMode, LoraConfig, LoraMode, LoraRegion, SpreadingFactor},
    *,
};
use embassy_stm32::{
    dbgmcu::Dbgmcu,
    dma::NoDma,
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pin, Pull, Speed},
    interrupt, pac,
    peripherals::{PA0, PB11, PB15, PB9, RNG},
    rcc,
    rng::Rng,
    subghz::*,
    Peripherals,
};

mod app;

use app::*;

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));

type Led1 = Led<Output<'static, PB15>>;
type Led2 = Led<Output<'static, PB9>>;
type Led3 = Led<Output<'static, PB11>>;

type LoraDriver = LoraDevice<'static, SubGhzRadio<'static>, Rng<RNG>>; //, SubGhzRadioIrq<'static>>;
type MyApp = App<Address<'static, LoraActor<LoraDriver>>, Led1, Led2, Led3>;

pub struct MyDevice {
    lora: ActorContext<'static, LoraActor<LoraDriver>>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PA0>, MyApp>>,
    app: ActorContext<'static, MyApp>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn config() -> embassy_stm32::Config {
    let mut config = embassy_stm32::Config::default();
    config.rcc = config.rcc.clock_src(embassy_stm32::rcc::ClockSrc::HSI16);
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
        let mut rcc = rcc::Rcc::new(p.RCC);
        rcc.enable_lsi();
        pac::RCC.ccipr().modify(|w| {
            w.set_rngsel(0b01);
        });
    }

    let led1 = Led::new(Output::new(p.PB15, Level::Low, Speed::Low));
    let led2 = Led::new(Output::new(p.PB9, Level::Low, Speed::Low));
    let led3 = Led::new(Output::new(p.PB11, Level::Low, Speed::Low));

    let button = Input::new(p.PA0, Pull::Up);
    let pin = ExtiInput::new(button, p.EXTI0);

    let ctrl1 = Output::new(p.PC3.degrade(), Level::High, Speed::High);
    let ctrl2 = Output::new(p.PC4.degrade(), Level::High, Speed::High);
    let ctrl3 = Output::new(p.PC5.degrade(), Level::High, Speed::High);
    let mut rfs = RadioSwitch::new(ctrl1, ctrl2, ctrl3);
    rfs.set_rx();

    let radio = SubGhz::new(p.SUBGHZSPI, p.PA5, p.PA7, p.PA6, NoDma, NoDma);

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

    static mut RADIO_STATE: SubGhzState<'static> = SubGhzState::new();
    let irq = interrupt::take!(SUBGHZ_RADIO);
    static mut RADIO_BUFFER: [u8; 256] = [0; 256];
    let lora = unsafe {
        LoraDevice::new(
            &config,
            SubGhzRadio::new(&mut RADIO_STATE, radio, rfs, irq),
            Rng::new(p.RNG),
            &mut RADIO_BUFFER,
        )
        .unwrap()
    };

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::new(join_mode, led1, led2, led3)),
        lora: ActorContext::new(LoraActor::new(lora)),
        button: ActorContext::new(Button::new(pin)),
    });

    DEVICE
        .mount(|device| async move {
            let lora = device.lora.mount((), spawner);
            let app = device.app.mount(lora, spawner);
            device.button.mount(app, spawner);
        })
        .await;
}
