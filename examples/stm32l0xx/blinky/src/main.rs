#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use log::LevelFilter;
use panic_probe as _;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Trace);

use embassy::{executor::Spawner, time::*};
use embassy_stm32::gpio::{Input, Level, Output, Pull};
use embassy_stm32::Peripherals;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use stm32l0::stm32l0x2 as pac;

#[embassy::main(config = "embassy_stm32::Config::new(embassy_stm32::rcc::Config::hsi16())")]
async fn main(spawner: Spawner, p: Peripherals) {
    rtt_init_print!();
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
    }

    log::set_max_level(log::LevelFilter::Trace);
    log::info!("Hello World!");

    let pp = pac::Peripherals::take().unwrap();

    pp.DBG.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });
    pp.RCC.ahbenr.modify(|_, w| w.dmaen().enabled());

    let mut led = Output::new(p.PB5, Level::Low);

    loop {
        log::info!("high!");

        led.set_high().unwrap();

        Timer::after(Duration::from_secs(1)).await;
        //cortex_m::asm::delay(1_000_000);

        log::info!("low!");

        led.set_low().unwrap();

        Timer::after(Duration::from_secs(1)).await;
        //Timer::after(Duration::from_secs(1)).await;
    }
}
