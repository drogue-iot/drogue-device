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
    actors::button::*,
    drivers::led::*,
    drivers::lora::sx127x::*,
    stm32::{
        exti::ExtiInput,
        gpio::{AnyPin, Input, Pin, Pull},
        interrupt,
        pac::*,
        peripherals::{PB2, PB4},
    },
    traits::{gpio::WaitForRisingEdge, lora::*},
    *,
};

use stm32l0xx_hal as hal;

use hal::{
    delay::Delay,
    gpio::{
        gpioa::{PA15, PA5, PA6, PA7},
        gpiob::{PB3, PB5, PB6, PB7},
        gpioc::PC0,
        Analog, Output, PullUp, PushPull,
    },
    pac::Peripherals as HalPeripherals,
    pac::SPI1,
    prelude::*,
    rcc,
    rng::Rng,
    spi, syscfg,
};

use embedded_hal::digital::v2::InputPin;

mod app;
mod lora;

use app::*;
use lora::*;

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));
static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Trace);

static mut RNG: Option<Rng> = None;
fn get_random_u32() -> u32 {
    unsafe {
        if let Some(rng) = &mut RNG {
            // enable starts the ADC conversions that generate the random number
            rng.enable();
            // wait until the flag flips; interrupt driven is possible but no implemented
            rng.wait();
            // reading the result clears the ready flag
            let val = rng.take_result();
            // can save some power by disabling until next random number needed
            rng.disable();
            val
        } else {
            panic!("No Rng exists!");
        }
    }
}

pub type Sx127x<'a> = Sx127xDriver<
    'a,
    ExtiInput<'a, PB4>,
    spi::Spi<SPI1, (PB3<Analog>, PA6<Analog>, PA7<Analog>)>,
    PA15<Output<PushPull>>,
    PC0<Output<PushPull>>,
    spi::Error,
>;

type Led1Pin = PB5<Output<PushPull>>;
type Led2Pin = PA5<Output<PushPull>>;
type Led3Pin = PB6<Output<PushPull>>;
type Led4Pin = PB7<Output<PushPull>>;

type MyApp = App<Sx127x<'static>, Led4Pin, Led2Pin, Led3Pin, Led1Pin>;

pub struct MyDevice {
    lora: ActorContext<'static, LoraActor<Sx127x<'static>>>,

    button: ActorContext<'static, Button<'static, ExtiInput<'static, PB2>, MyApp>>,
    app: ActorContext<'static, MyApp>,
}

#[drogue::main(
    config = "drogue_device::stm32::Config::default().rcc(drogue_device::stm32::rcc::Config::default().clock_src(drogue_device::stm32::rcc::ClockSrc::HSI16))"
)]
async fn main(context: DeviceContext<MyDevice>, p: Peripherals) {
    rtt_init_print!();
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }

    log::set_max_level(log::LevelFilter::Trace);
    let device = unsafe { HalPeripherals::steal() };

    // NEEDED FOR RTT
    device.DBG.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });
    device.RCC.ahbenr.modify(|_, w| w.dmaen().enabled());
    // NEEDED FOR RTT

    // TODO: This must be in sync with above, but is there a
    // way we can get hold of rcc without freezing twice?
    let mut rcc = device.RCC.freeze(rcc::Config::hsi16());

    let mut syscfg = syscfg::SYSCFG::new(device.SYSCFG, &mut rcc);
    let hsi48 = rcc.enable_hsi48(&mut syscfg, device.CRS);
    unsafe { RNG.replace(Rng::new(device.RNG, &mut rcc, hsi48)) };

    let gpioa = device.GPIOA.split(&mut rcc);
    let gpiob = device.GPIOB.split(&mut rcc);
    let gpioc = device.GPIOC.split(&mut rcc);

    let led1 = Led::new(gpiob.pb5.into_push_pull_output());
    let led2 = Led::new(gpioa.pa5.into_push_pull_output());
    let led3 = Led::new(gpiob.pb6.into_push_pull_output());
    let led4 = Led::new(gpiob.pb7.into_push_pull_output());

    let button = Input::new(p.PB2, Pull::Up);
    let mut pin = ExtiInput::new(button, p.EXTI2);

    // SPI for sx127x
    let spi = device.SPI1.spi(
        (gpiob.pb3, gpioa.pa6, gpioa.pa7),
        spi::MODE_0,
        200_000.hz(),
        &mut rcc,
    );
    let cs = gpioa.pa15.into_push_pull_output();
    let reset = gpioc.pc0.into_push_pull_output();
    let _ = gpiob.pb1.into_floating_input();

    let ready = Input::new(p.PB4, Pull::Up);
    let ready_pin = ExtiInput::new(ready, p.EXTI4);

    let cdevice = cortex_m::Peripherals::take().unwrap();
    let mut delay = Delay::new(cdevice.SYST, rcc.clocks);

    let lora = Sx127xDriver::new(ready_pin, spi, cs, reset, &mut delay, get_random_u32)
        .expect("error creating sx127x driver");

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .device_eui(&DEV_EUI.trim_end().into())
        .app_eui(&APP_EUI.trim_end().into())
        .app_key(&APP_KEY.trim_end().into());

    log::info!("Configuring with config {:?}", config);

    context.configure(MyDevice {
        app: ActorContext::new(App::new(AppInitConfig {
            tx_led: led2,
            green_led: led1,
            init_led: led4,
            user_led: led3,
            lora: Some(config),
        })),
        lora: ActorContext::new(LoraActor::new(lora)),
        button: ActorContext::new(Button::new(pin)),
    });

    /*
    print_size::<LoraActor<Sx127x<'static>>>("LoraActor");
    print_size::<ActorContext<'static, LoraActor<Sx127x<'static>>>>("ActorContext<LoraActor>");
    print_size::<ActorContext<'static, Led<Led1Pin>>>("ActorContext<Led1Pin>");
    print_size::<Led<Led1Pin>>("Led<Led1Pin>");
    print_size::<Led1Pin>("Led1Pin");
    */

    context.mount(|device, spawner| {
        let lora = device.lora.mount((), spawner);
        let app = device.app.mount(AppConfig { lora }, spawner);
        device.button.mount(app, spawner);
    });
}
