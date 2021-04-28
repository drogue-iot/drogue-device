#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
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
    drivers::lora::sx127x::*,
    stm32::{
        exti::ExtiPin,
        hal::{
            delay::Delay,
            gpio::{
                gpioa::{PA15, PA6, PA7, PA5},
                gpiob::{PB2, PB3, PB4, PB5, PB6, PB7},
                gpioc::PC0,
                Analog, Input, Output, PullUp, PushPull,
            },
            pac::Peripherals,
            pac::SPI1,
            prelude::*,
            rcc,
            rng::Rng,
            spi, syscfg,
        },
        interrupt,
    },
    traits::lora::*,
    *,
};

mod app;
use app::*;
use drogue_device::actors::led::Led;

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
    ExtiPin<PB4<Input<PullUp>>>,
    spi::Spi<SPI1, (PB3<Analog>, PA6<Analog>, PA7<Analog>)>,
    PA15<Output<PushPull>>,
    PC0<Output<PushPull>>,
    spi::Error,
>;

type Led1Pin = PB5<Output<PushPull>>;
type Led2Pin = PA5<Output<PushPull>>;
type Led3Pin = PB6<Output<PushPull>>;
type Led4Pin = PB7<Output<PushPull>>;

#[derive(Device)]
pub struct MyDevice {
    button: ActorContext<'static, Button<'static, ExtiPin<PB2<Input<PullUp>>>, App<Sx127x<'static>, Led1Pin, Led2Pin, Led3Pin,Led4Pin>>>,
    app: ActorContext<'static, App<Sx127x<'static>, Led1Pin, Led2Pin, Led3Pin,Led4Pin>>,
    led1: ActorContext<'static, Led<Led1Pin>>,
    led2: ActorContext<'static, Led<Led2Pin>>,
    led3: ActorContext<'static, Led<Led3Pin>>,
    led4: ActorContext<'static, Led<Led4Pin>>,
}

#[drogue::main(config = "embassy_stm32::hal::rcc::Config::hsi16()")]
async fn main(context: DeviceContext<MyDevice>) {
    rtt_init_print!();
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }

    log::set_max_level(log::LevelFilter::Trace);
    let device = unsafe { Peripherals::steal() };

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

    let irq = interrupt::take!(EXTI2_3);

    let gpioa = device.GPIOA.split(&mut rcc);
    let gpiob = device.GPIOB.split(&mut rcc);
    let gpioc = device.GPIOC.split(&mut rcc);

    let button = gpiob.pb2.into_pull_up_input();

    let led1 = gpiob.pb5.into_push_pull_output();
    let led2 = gpioa.pa5.into_push_pull_output();
    let led3 = gpiob.pb6.into_push_pull_output();
    let led4 = gpiob.pb7.into_push_pull_output();

    let pin = ExtiPin::new(button, irq, &mut syscfg);

    // SPI for sx127x
    let spi = device.SPI1.spi(
        (gpiob.pb3, gpioa.pa6, gpioa.pa7),
        spi::MODE_0,
        200_000.hz(),
        &mut rcc,
    );
    let cs = gpioa.pa15.into_push_pull_output();
    let reset = gpioc.pc0.into_push_pull_output();
    let ready = gpiob.pb4.into_pull_up_input();
    let _ = gpiob.pb1.into_floating_input();

    let ready_irq = interrupt::take!(EXTI4_15);
    let ready_pin = ExtiPin::new(ready, ready_irq, &mut syscfg);

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
        app: ActorContext::new(App::new(lora, config)),
        button: ActorContext::new(Button::new(pin)),
        led1: ActorContext::new(Led::new(led1)),
        led2: ActorContext::new(Led::new(led2)),
        led3: ActorContext::new(Led::new(led3)),
        led4: ActorContext::new(Led::new(led4)),
    });

    context.mount(|device| {
        let led1 = device.led1.mount(());
        let led2 = device.led2.mount(());
        let led3 = device.led3.mount(());
        let led4 = device.led4.mount(());
        let app = device.app.mount(AppConfig { led1, led2, led3, led4 });
                device.button.mount(app);
    });
}
