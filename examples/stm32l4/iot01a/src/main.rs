#![no_std]
#![no_main]

mod device;
use device::MyDevice;

use cortex_m_rt::{entry, exception};
use stm32l4xx_hal::{
    gpio::Edge,
    i2c::I2c as HalI2c,
    pac::interrupt::{EXTI15_10, TIM15},
    prelude::*,
    rcc::RccExt,
    stm32::Peripherals,
    time::Hertz,
};

use log::LevelFilter;
use panic_rtt_target as _;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use drogue_device::driver::spi::Spi;
use drogue_device::driver::wifi::eswifi::EsWifi;
use drogue_device::{
    domain::time::duration::Milliseconds,
    driver::{
        button::Button,
        i2c::I2c,
        led::{Blinker, SimpleLED},
        memory::Memory,
        sensor::hts221::Hts221,
        timer::Timer,
    },
    port::stm32l4xx::timer::Timer as McuTimer,
    hal::Active,
    prelude::*,
};
use embedded_hal::spi::{Mode, MODE_0};
use stm32l4xx_hal::pac::Interrupt::{EXTI1, SPI3};
use stm32l4xx_hal::spi::Spi as HalSpi;
use stm32l4xx_hal::time::MegaHertz;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Debug);

#[entry]
fn main() -> ! {
    rtt_init_print!();
    //rtt_init_print!(BlockIfFull);
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    let mut device = Peripherals::take().unwrap();

    log::info!("[main] Initializing");
    let mut flash = device.FLASH.constrain();
    let mut rcc = device.RCC.constrain();
    let mut pwr = device.PWR.constrain(&mut rcc.apb1r1);
    let clocks = rcc
        .cfgr
        .sysclk(80.mhz())
        .pclk1(80.mhz())
        .pclk2(80.mhz())
        .freeze(&mut flash.acr, &mut pwr);

    let mut gpioa = device.GPIOA.split(&mut rcc.ahb2);
    let mut gpiob = device.GPIOB.split(&mut rcc.ahb2);
    let mut gpioc = device.GPIOC.split(&mut rcc.ahb2);
    let mut gpiod = device.GPIOD.split(&mut rcc.ahb2);
    let mut gpioe = device.GPIOE.split(&mut rcc.ahb2);

    // == LEDs ==

    let ld1 = gpioa
        .pa5
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

    let ld1 = SimpleLED::new(ld1, Active::High);

    let ld2 = gpiob
        .pb14
        .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

    let ld2 = SimpleLED::new(ld2, Active::High);

    // == Blinker ==

    let blinker1 = Blinker::new(Milliseconds(500u32));
    let blinker2 = Blinker::new(Milliseconds(1000u32));

    // == Button ==

    let mut button = gpioc
        .pc13
        .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);

    button.make_interrupt_source(&mut device.SYSCFG, &mut rcc.apb2);
    button.enable_interrupt(&mut device.EXTI);
    button.trigger_on_edge(&mut device.EXTI, Edge::RISING_FALLING);

    let button = Button::new(button, Active::Low);

    // == i2c

    let scl = gpiob
        .pb10
        .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper)
        .into_af4(&mut gpiob.moder, &mut gpiob.afrh);

    let sda = gpiob
        .pb11
        .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper)
        .into_af4(&mut gpiob.moder, &mut gpiob.afrh);

    let i2c = HalI2c::i2c2(
        device.I2C2,
        (scl, sda),
        Hertz(100_000u32),
        clocks,
        &mut rcc.apb1r1,
    );

    // Create the Actor around the i2c bus.
    let i2c = I2c::new(i2c);

    // == HTS221 ==

    let mut ready = gpiod
        .pd15
        .into_pull_down_input(&mut gpiod.moder, &mut gpiod.pupdr);

    ready.enable_interrupt(&mut device.EXTI);
    ready.make_interrupt_source(&mut device.SYSCFG, &mut rcc.apb2);
    ready.trigger_on_edge(&mut device.EXTI, Edge::RISING);

    // Create the Actor around the HTS221
    let hts221 = Hts221::new(ready, EXTI15_10);

    // == Timer ==

    let mcu_timer = McuTimer::tim15(device.TIM15, clocks, &mut rcc.apb2);
    let timer = Timer::new(mcu_timer, TIM15);

    // == SPI ==

    let clk = gpioc.pc10.into_af6(&mut gpioc.moder, &mut gpioc.afrh);
    let miso = gpioc.pc11.into_af6(&mut gpioc.moder, &mut gpioc.afrh);
    let mosi = gpioc.pc12.into_af6(&mut gpioc.moder, &mut gpioc.afrh);

    let spi = HalSpi::spi3(
        device.SPI3,
        (clk, miso, mosi),
        MODE_0,
        MegaHertz(20),
        clocks,
        &mut rcc.apb1r1,
    );

    let spi = Spi::new(spi);

    // == Wifi ==

    let wifi_cs = gpioe
        .pe0
        .into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let mut wifi_ready = gpioe
        .pe1
        //.into_pull_up_input(&mut gpioe.moder, &mut gpioe.pupdr);
        .into_pull_down_input(&mut gpioe.moder, &mut gpioe.pupdr);

    wifi_ready.enable_interrupt(&mut device.EXTI);
    wifi_ready.make_interrupt_source(&mut device.SYSCFG, &mut rcc.apb2);
    wifi_ready.trigger_on_edge(&mut device.EXTI, Edge::RISING_FALLING);

    let mut wifi_reset = gpioe
        .pe8
        .into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let mut wifi_wakeup = gpiob
        .pb13
        .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

    let wifi = EsWifi::new(wifi_cs, wifi_ready, EXTI1, wifi_reset, wifi_wakeup);

    // == Device ==

    let device = MyDevice {
        spi,
        wifi,
        memory: ActorContext::new(Memory::new()).with_name("memory"),
        ld1: ActorContext::new(ld1).with_name("ld1"),
        ld2: ActorContext::new(ld2).with_name("ld2"),
        blinker1: ActorContext::new(blinker1).with_name("blinker1"),
        blinker2: ActorContext::new(blinker2).with_name("blinker2"),
        i2c,
        hts221,
        button: InterruptContext::new(button, EXTI15_10).with_name("button"),
        timer,
    };

    device!( MyDevice = device; 2048 );
}
