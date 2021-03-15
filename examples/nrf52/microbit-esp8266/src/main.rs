#![no_main]
#![no_std]

mod app;
mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    api::ip::IpAddress,
    driver::button::*,
    driver::memory::Memory,
    driver::timer::Timer,
    driver::uart::serial::*,
    driver::wifi::esp8266::Esp8266Wifi,
    hal::Active,
    platform::cortex_m::nrf::{
        gpiote::*,
        timer::Timer as HalTimer,
        uarte::{Baudrate, Parity, Pins, Uarte},
    },
    prelude::*,
};
use hal::gpio::Level;
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use nrf52833_hal as hal;

use crate::app::*;
use crate::device::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const ENDPOINT: IpAddress = IpAddress::new_v4(192, 168, 1, 2);
const ENDPOINT_PORT: u16 = 12345;

fn configure() -> MyDevice {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(device.P0);

    let clocks = hal::clocks::Clocks::new(device.CLOCK).enable_ext_hfosc();
    let _clocks = clocks.start_lfclk();

    // Buttons
    let button_a = port0.p0_14.into_pullup_input().degrade();
    let button_b = port0.p0_23.into_pullup_input().degrade();

    let gpiote = hal::gpiote::Gpiote::new(device.GPIOTE);
    gpiote
        .channel0()
        .input_pin(&button_a)
        .hi_to_lo()
        .enable_interrupt();
    gpiote
        .channel1()
        .input_pin(&button_b)
        .hi_to_lo()
        .enable_interrupt();

    // Timer
    let timer = Timer::new(HalTimer::new(device.TIMER0), hal::pac::Interrupt::TIMER0);

    // Uart

    static mut RX_BUF: [u8; 1] = [0; 1];
    let (tx, rx) = Uarte::new(
        device.UARTE0,
        Pins {
            txd: port0.p0_01.into_push_pull_output(Level::High).degrade(),
            rxd: port0.p0_13.into_floating_input().degrade(),
            cts: None,
            rts: None,
        },
        Parity::EXCLUDED,
        Baudrate::BAUD115200,
    )
    .split(unsafe { &mut RX_BUF });

    let uart = Serial::new(tx, rx, hal::pac::Interrupt::UARTE0_UART0);

    // Wifi
    let enable_pin = port0.p0_03.into_push_pull_output(Level::Low).degrade();
    let reset_pin = port0.p0_02.into_push_pull_output(Level::Low).degrade();

    MyDevice {
        btn_connect: ActorContext::new(Button::new(button_a, Active::Low))
            .with_name("button_connect"),
        btn_send: ActorContext::new(Button::new(button_b, Active::Low)).with_name("button_send"),
        gpiote: InterruptContext::new(Gpiote::new(gpiote), hal::pac::Interrupt::GPIOTE)
            .with_name("gpiote"),
        uart,
        memory: ActorContext::new(Memory::new()).with_name("memory"),
        wifi: Esp8266Wifi::new(enable_pin, reset_pin),
        timer,
        app: ActorContext::new(App::new(WIFI_SSID, WIFI_PSK, ENDPOINT, ENDPOINT_PORT))
            .with_name("app"),
    }
}

#[entry]
fn main() -> ! {
    device!(MyDevice = configure; 12000);
}
