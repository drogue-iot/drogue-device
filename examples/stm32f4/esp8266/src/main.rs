#![no_main]
#![no_std]

const WIFI_SSID: &str = include_str!("wifi.ssid.txt");
const WIFI_PSK: &str = include_str!("wifi.password.txt");
const ENDPOINT: IpAddress = IpAddress::new_v4(192, 168, 1, 2);
const ENDPOINT_PORT: u16 = 12345;

mod app;
mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    api::ip::IpAddress,
    driver::uart::serial::*,
    driver::{button::Button, wifi::esp8266::Esp8266Wifi},
    hal::Active,
    prelude::*,
};
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use hal::gpio::Edge;
use hal::serial::{
    config::{Config, Parity, StopBits},
    Serial as NucleoSerial,
};
use stm32f4xx_hal as hal;
use stm32f4xx_hal::gpio::ExtiPin;
use stm32f4xx_hal::prelude::*;

use crate::app::*;
use crate::device::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

fn configure() -> MyDevice {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let mut device = hal::stm32::Peripherals::take().unwrap();

    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(84.mhz()).freeze();

    let gpioa = device.GPIOA.split();
    let gpioc = device.GPIOC.split();

    let pa11 = gpioa.pa11;
    let pa12 = gpioa.pa12;

    // SERIAL pins for USART6
    let tx_pin = pa11.into_alternate_af8();
    let rx_pin = pa12.into_alternate_af8();

    // enable pin
    let en = gpioc.pc10.into_push_pull_output();
    // reset pin
    let reset = gpioc.pc12.into_push_pull_output();

    let usart6 = device.USART6;

    let mut serial = NucleoSerial::usart6(
        usart6,
        (tx_pin, rx_pin),
        Config {
            baudrate: 115_200.bps(),
            parity: Parity::ParityNone,
            stopbits: StopBits::STOP1,
            ..Default::default()
        },
        clocks,
    )
    .unwrap();

    serial.listen(hal::serial::Event::Rxne);
    let (tx, rx) = serial.split();

    let uart = Serial::new(tx, rx, hal::stm32::Interrupt::USART6);
    let wifi = Esp8266Wifi::new(en, reset);

    let timer = AppTimer::new(DummyTimer {}, hal::stm32::Interrupt::TIM2);

    // Create a button input with an interrupt
    let mut button = gpioc.pc13.into_pull_up_input();
    button.make_interrupt_source(&mut device.SYSCFG.constrain());
    button.enable_interrupt(&mut device.EXTI);
    button.trigger_on_edge(&mut device.EXTI, Edge::FALLING);

    let button = Button::new(button, Active::Low);

    MyDevice {
        button: InterruptContext::new(button, hal::stm32::Interrupt::EXTI15_10).with_name("button"),
        timer,
        uart,
        wifi,
        app: ActorContext::new(App::new(WIFI_SSID, WIFI_PSK, ENDPOINT, ENDPOINT_PORT))
            .with_name("app"),
    }
}

#[entry]
fn main() -> ! {
    device!(MyDevice = configure; 12000);
}
