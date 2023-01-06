#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {
    core::{convert::Infallible, fmt::Write as _},
    embassy_executor::{Executor, Spawner},
    embassy_time::{Duration, Timer},
    embedded_hal_1::spi::ErrorType,
    embedded_hal_async::spi::{ExclusiveDevice, SpiBusFlush, SpiBusRead, SpiBusWrite},
    embedded_nal_async::*,
    esp32c3_hal::{
        clock::ClockControl, embassy, peripherals::Peripherals, prelude::*, timer::TimerGroup, Rtc,
    },
    static_cell::StaticCell,
};

use esp_backtrace as _;

//const WIFI_SSID: &str = drogue::config!("wifi-ssid");
//const WIFI_PSK: &str = drogue::config!("wifi-password");
//
//#[path = "../../../../common/dns.rs"]
//mod dns;
//use dns::*;
//
//#[path = "../../../../common/temperature.rs"]
//mod temperature;
//use temperature::*;
//
///// HTTP endpoint hostname
//const HOSTNAME: &str = drogue::config!("hostname");
//
///// HTTP endpoint port
//const PORT: &str = drogue::config!("port");
//
///// HTTP username
//const USERNAME: &str = drogue::config!("username");
//
///// HTTP password
//const PASSWORD: &str = drogue::config!("password");

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

#[embassy_executor::task]
async fn run() {
    loop {
        esp_println::println!("Hello world from embassy using esp-hal-async!");
        Timer::after(Duration::from_millis(1_000)).await;
    }
}

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[riscv_rt::entry]
fn main() -> ! {
    esp_println::println!("Init!");
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let mut rtc = Rtc::new(peripherals.RTC_CNTL);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
    let mut wdt1 = timer_group1.wdt;

    // Disable watchdog timers
    rtc.swd.disable();
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();

    #[cfg(feature = "embassy-time-systick")]
    embassy::init(
        &clocks,
        esp32c3_hal::systimer::SystemTimer::new(peripherals.SYSTIMER),
    );

    #[cfg(feature = "embassy-time-timg0")]
    embassy::init(&clocks, timer_group0.timer0);

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(run()).ok();
    });
}
