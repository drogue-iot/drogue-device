#![no_main]
#![no_std]

mod app;
mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    api::ip::IpAddress, driver::uart::serial::*, driver::wifi::esp8266::Esp8266Wifi, prelude::*,
};
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use nucleo_f401re::{
    hal,
    hal::{
        prelude::*,
        serial::{
            config::{Config, Parity, StopBits},
            Rx, Serial as NucleoSerial, Tx,
        },
    },
    pac::USART6,
};

use crate::app::*;
use crate::device::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

const WIFI_SSID: &str = include_str!("wifi.ssid.txt");
const WIFI_PSK: &str = include_str!("wifi.password.txt");
const ENDPOINT: IpAddress = IpAddress::new_v4(192, 168, 1, 2);
const ENDPOINT_PORT: u16 = 12345;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::stm32::Peripherals::take().unwrap();

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
    let mut en = gpioc.pc10.into_push_pull_output();
    // reset pin
    let mut reset = gpioc.pc12.into_push_pull_output();

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

    device!( MyDevice = MyDevice{
        uart,
        wifi,

    }; 12000);
}
