#![no_main]
#![no_std]

mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    api::lora::*,
    driver::button::*,
    driver::lora::*,
    driver::memory::Memory,
    driver::timer::Timer,
    driver::uart::dma::DmaUart,
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

use crate::device::*;
use lora_common::*;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

const DEV_EUI: &str = include_str!("dev_eui.txt");
const APP_EUI: &str = include_str!("app_eui.txt");
const APP_KEY: &str = include_str!("app_key.txt");

#[entry]
fn main() -> ! {
    //rtt_init_print!();
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let device = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(device.P0);
    let port1 = hal::gpio::p1::Parts::new(device.P1);

    let clocks = hal::clocks::Clocks::new(device.CLOCK).enable_ext_hfosc();
    let _clocks = clocks.start_lfclk();

    let gpiote = hal::gpiote::Gpiote::new(device.GPIOTE);

    // GPIO button
    let button_pin = port0.p0_14.into_pullup_input().degrade();
    gpiote
        .channel0()
        .input_pin(&button_pin)
        .hi_to_lo()
        .enable_interrupt();

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

    let device = MyDevice {
        button: ActorContext::new(Button::new(button_pin, Active::Low)).with_name("button"),
        gpiote: InterruptContext::new(Gpiote::new(gpiote), hal::pac::Interrupt::GPIOTE)
            .with_name("gpiote"),
        uart,
        lora: rak811::Rak811::new(port1.p1_02.into_push_pull_output(Level::High).degrade()),
        memory: ActorContext::new(Memory::new()).with_name("memory"),
        timer,
        app: ActorContext::new(App::new(
            LoraConfig::new()
                .band(LoraRegion::EU868)
                .lora_mode(LoraMode::WAN)
                .device_eui(&DEV_EUI.into())
                .app_eui(&APP_EUI.into())
                .app_key(&APP_KEY.into()),
        ))
        .with_name("application"),
    };

    device!( MyDevice = device; 8192);
}
