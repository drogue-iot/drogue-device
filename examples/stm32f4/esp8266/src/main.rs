#![no_main]
#![no_std]

mod app;
mod device;

use panic_rtt_target as _;

use cortex_m_rt::{entry, exception};
use drogue_device::{api::ip::IpAddress, prelude::*};
use log::LevelFilter;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use stm32f4xx_hal as hal;

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

    device!( MyDevice = MyDevice{}; 12000);
}
