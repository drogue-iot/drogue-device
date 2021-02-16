#![no_main]
#![no_std]

mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    domain::time::rate::Extensions,
    driver::timer::Timer,
    driver::uart::dma::DmaUart,
    port::nrf::timer::Timer as HalTimer,
    port::nrf::{gpiote::*, uarte::{Baudrate, Parity, Pins, Uarte}},
    prelude::*,
};
use hal::gpio::Level;
use heapless::{consts, Vec};
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use nrf52833_hal as hal;

use crate::device::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

#[entry]
fn main() -> ! {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(device.P0);
    let port1 = hal::gpio::p1::Parts::new(device.P1);

    let clocks = hal::clocks::Clocks::new(device.CLOCK).enable_ext_hfosc();
    let _clocks = clocks.start_lfclk();

    let gpiote = Gpiote::new(device.GPIOTE);

    // GPIO channels
    let button_fwd = gpiote.configure_channel(
        Channel::Channel0,
        port0.p0_14.into_pullup_input().degrade(),
        Edge::Falling,
    );
    let button_back = gpiote.configure_channel(
        Channel::Channel1,
        port0.p0_23.into_pullup_input().degrade(),
        Edge::Falling,
    );

    // Timer
    let timer = Timer::new(HalTimer::new(device.TIMER0), hal::pac::Interrupt::TIMER0);

    // Uart
    let uart = DmaUart::new(
        Uarte::new(
            device.UARTE0,
            Pins {
                txd: port0.p0_01.into_push_pull_output(Level::High).degrade(),
                rxd: port0.p0_13.into_floating_input().degrade(),
                cts: None,
                rts: None,
            },
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        ),
        hal::pac::Interrupt::UARTE0_UART0,
    );

    // LED Matrix
    let mut rows = Vec::<_, consts::U5>::new();
    rows.push(port0.p0_21.into_push_pull_output(Level::Low).degrade())
        .ok();
    rows.push(port0.p0_22.into_push_pull_output(Level::Low).degrade())
        .ok();
    rows.push(port0.p0_15.into_push_pull_output(Level::Low).degrade())
        .ok();
    rows.push(port0.p0_24.into_push_pull_output(Level::Low).degrade())
        .ok();
    rows.push(port0.p0_19.into_push_pull_output(Level::Low).degrade())
        .ok();

    let mut cols = Vec::<_, consts::U5>::new();
    cols.push(port0.p0_28.into_push_pull_output(Level::Low).degrade())
        .ok();
    cols.push(port0.p0_11.into_push_pull_output(Level::Low).degrade())
        .ok();
    cols.push(port0.p0_31.into_push_pull_output(Level::Low).degrade())
        .ok();
    cols.push(port1.p1_05.into_push_pull_output(Level::Low).degrade())
        .ok();
    cols.push(port0.p0_30.into_push_pull_output(Level::Low).degrade())
        .ok();

    // Set refresh rate to avoid led flickering
    let led = LedMatrix::new(rows, cols, 200u32.Hz());

    let device = MyDevice {
        btn_fwd: ActorContext::new(button_fwd).with_name("button a"),
        btn_back: ActorContext::new(button_back),
        gpiote: InterruptContext::new(gpiote, hal::pac::Interrupt::GPIOTE).with_name("gpiote"),
        led: ActorContext::new(led).with_name("matrix"),
        timer,
        uart,
        app: ActorContext::new(App::new()),
    };

    device!( MyDevice = device; 4096);
}
