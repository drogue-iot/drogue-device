#![no_main]
#![no_std]

mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    domain::time::rate::Extensions,
    //    driver::timer::Timer,
    driver::uart::{serial_rx::*, serial_tx::*},
    platform::cortex_m::nrf::uarte::{Baudrate, Parity, Pins, Uarte},
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

fn configure() -> MyDevice {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(device.P0);
    let port1 = hal::gpio::p1::Parts::new(device.P1);

    let clocks = hal::clocks::Clocks::new(device.CLOCK).enable_ext_hfosc();
    let _clocks = clocks.start_lfclk();

    // Timer
    // let timer = Timer::new(HalTimer::new(device.TIMER0), hal::pac::Interrupt::TIMER0);

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
    let tx = SerialTx::new(tx);
    let rx = SerialRx::new(rx);

    /*
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
    */

    MyDevice {
        //   led: ActorContext::new(led).with_name("matrix"),
        //        timer,
        tx: ActorContext::new(tx).with_name("uart_tx"),
        rx: InterruptContext::new(rx, hal::pac::Interrupt::UARTE0_UART0).with_name("uart_rx"),
        app: ActorContext::new(App::new()),
    }
}

#[entry]
fn main() -> ! {
    device!(MyDevice = configure; 8192);
}
