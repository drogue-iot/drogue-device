#![no_main]
#![no_std]

mod app;
mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    driver::memory::Memory,
    driver::timer::Timer,
    driver::uart::serial::*,
    driver::wifi::esp8266::Esp8266Wifi,
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

#[entry]
fn main() -> ! {
    //rtt_init_print!();
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(device.P0);

    let clocks = hal::clocks::Clocks::new(device.CLOCK).enable_ext_hfosc();
    let _clocks = clocks.start_lfclk();

    let gpiote = Gpiote::new(device.GPIOTE);

    // GPIO channels
    let btn_connect = gpiote.configure_channel(
        Channel::Channel0,
        port0.p0_14.into_pullup_input().degrade(),
        Edge::Falling,
    );

    let btn_send = gpiote.configure_channel(
        Channel::Channel1,
        port0.p0_23.into_pullup_input().degrade(),
        Edge::Falling,
    );

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

    let device = MyDevice {
        btn_connect: ActorContext::new(btn_connect).with_name("button_connect"),
        btn_send: ActorContext::new(btn_send).with_name("button_send"),
        gpiote: InterruptContext::new(gpiote, hal::pac::Interrupt::GPIOTE).with_name("gpiote"),
        uart,
        memory: ActorContext::new(Memory::new()).with_name("memory"),
        wifi: Esp8266Wifi::new(enable_pin, reset_pin),
        timer,
        app: ActorContext::new(App::new()).with_name("app"),
    };

    device!( MyDevice = device; 12000);
}
