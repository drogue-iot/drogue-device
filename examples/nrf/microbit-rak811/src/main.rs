#![no_main]
#![no_std]

mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{
    driver::gpiote::nrf::*,
    driver::lora::*,
    driver::uart::Uart,
    hal::uart::nrf::{Baudrate, Parity, Pins, Uarte as HalUart},
    prelude::*,
};
use hal::gpio::Level;
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

    // Uart
    let uart = Uart::new(
        HalUart::new(
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

    let device = LoraDevice {
        btn_connect: ActorContext::new(btn_connect).with_name("button_connect"),
        btn_send: ActorContext::new(btn_send).with_name("button_send"),
        gpiote: InterruptContext::new(gpiote, hal::pac::Interrupt::GPIOTE).with_name("gpiote"),
        uart,
        lora: ActorContext::new(rak811::Rak811::new()),
        app: ActorContext::new(App::new(
            LoraConfig::new()
                .connect_mode(ConnectMode::OTAA)
                .band(LoraRegion::EU868)
                .lora_mode(LoraMode::WAN)
                .device_eui(&[0x00, 0xBB, 0x7C, 0x95, 0xAD, 0xB5, 0x30, 0xB9])
                .app_eui(&[0x70, 0xB3, 0xD5, 0x7E, 0xD0, 0x03, 0xB1, 0x84])
                .app_key(&[
                    0xE2, 0xB5, 0x25, 0xB6, 0x86, 0xB8, 0xE2, 0xE6, 0xFE, 0x22, 0x27, 0x51, 0xAF,
                    0x35, 0xCD, 0x22,
                ]),
        )),
    };

    device!( LoraDevice = device; 4096);
}
