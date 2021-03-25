#![no_main]
#![no_std]

mod device;
use panic_rtt_target as _;

use core::cell::RefCell;
use cortex_m_rt::{entry, exception};
use drogue_device::{
    api::lora::*,
    driver::{button::*, lora::sx127x::*, memory::Memory, spi::Spi, timer::Timer},
    hal::Active,
    platform::cortex_m::stm32l0xx::{gpio::Pin, timer::HardwareTimer},
    prelude::*,
    system::DeviceContext,
};
use log::LevelFilter;
use lora_common::*;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use stm32l0xx_hal::{
    delay::Delay,
    exti::Exti,
    exti::{ExtiLine, GpioLine, TriggerEdge},
    pac::interrupt::{EXTI2_3, EXTI4_15, TIM2},
    pac::Peripherals,
    prelude::*,
    rcc,
    rng::Rng,
    syscfg,
};

use crate::device::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));

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

fn configure() -> MyDevice {
    rtt_init_print!(BlockIfFull, 1024);
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }
    log::set_max_level(log::LevelFilter::Info);

    let cdevice = cortex_m::Peripherals::take().unwrap();
    let device = Peripherals::take().unwrap();
    let mut rcc = device.RCC.freeze(rcc::Config::hsi16());
    let mut syscfg = syscfg::SYSCFG::new(device.SYSCFG, &mut rcc);
    let gpioa = device.GPIOA.split(&mut rcc);
    let gpiob = device.GPIOB.split(&mut rcc);
    let gpioc = device.GPIOC.split(&mut rcc);

    let button = gpiob.pb2.into_pull_up_input();

    let mut exti = Exti::new(device.EXTI);

    let line = GpioLine::from_raw_line(button.pin_number()).unwrap();
    exti.listen_gpio(&mut syscfg, button.port(), line, TriggerEdge::Both);

    let hsi48 = rcc.enable_hsi48(&mut syscfg, device.CRS);
    unsafe { RNG.replace(Rng::new(device.RNG, &mut rcc, hsi48)) };

    // SPI to sx127x
    let spi = device.SPI1.spi(
        (gpiob.pb3, gpioa.pa6, gpioa.pa7),
        stm32l0xx_hal::spi::MODE_0,
        200_000.hz(),
        &mut rcc,
    );
    let cs = gpioa.pa15.into_push_pull_output();
    let reset = gpioc.pc0.into_push_pull_output();
    let ready = gpiob.pb4.into_pull_up_input();
    let busy = gpiob.pb1.into_floating_input();

    let ready_line = GpioLine::from_raw_line(ready.pin_number()).unwrap();
    exti.listen_gpio(&mut syscfg, ready.port(), ready_line, TriggerEdge::Rising);

    let mut delay = Delay::new(cdevice.SYST, rcc.clocks);

    // Configure the timer.
    let mcu_timer = HardwareTimer::tim2(device.TIM2, &mut rcc);
    let timer = Timer::new(mcu_timer, TIM2);

    let lora = Sx127x::new(
        spi,
        cs,
        reset,
        busy,
        delay,
        ready_line,
        EXTI4_15,
        get_random_u32,
    )
    .expect("error creating LoRa driver");

    MyDevice {
        button: InterruptContext::new(Button::new(Pin::new(button, line), Active::Low), EXTI2_3)
            .with_name("button"),
        timer,
        lora: lora,
        app: ActorContext::new(App::new(
            LoraConfig::new()
                .band(LoraRegion::EU868)
                .lora_mode(LoraMode::WAN)
                .device_eui(&DEV_EUI.into())
                .app_eui(&APP_EUI.into())
                .app_key(&APP_KEY.into()),
        ))
        .with_name("application"),
    }
}

#[entry]
fn main() -> ! {
    device!(MyDevice = configure; 3000);
}
