#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::led::*,
    actors::ticker::*,
//    actors::wifi::eswifi::*,
//    traits::{wifi::*},
    *};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::{
    gpio::{Level, Input, Output, Speed, Pull},
    peripherals:: {PA5, PB13, PB12},
    Peripherals,
};
use embassy_stm32::spi::{Config, Spi};
use embassy_stm32::time::Hertz;
//use defmt::*;
use embassy_stm32::dma::NoDma;
//use embedded_hal::digital::v2::{InputPin, OutputPin};

//use cortex_m::prelude::_embedded_hal_blocking_spi_Transfer;
use drogue_device::drivers::wifi::eswifi::EsWifiController;


type Led1Pin = Output<'static, PA5>;
type ENABLE = Output<'static, PB13>;
type RESET = Output<'static, PB12>;

pub struct MyDevice {
//    wifi: EsWifi<ENABLE, RESET>,
    led: ActorContext<'static, Led<Led1Pin>>,
    ticker: ActorContext<'static, Ticker<'static, Led<Led1Pin>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));

#[embassy::main]
async fn main(_spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    defmt::info!("Starting up...");

    let spi = Spi::new(
        p.SPI3,
        p.PC10,
        p.PC12,
        p.PC11,
        NoDma,
        NoDma,
        Hertz(1_000_000),
        Config::default(),
    );

    let _boot = Output::new(p.PB12, Level::Low, Speed::VeryHigh);
    let wake = Output::new(p.PB13, Level::Low, Speed::VeryHigh);
    let reset = Output::new(p.PE8, Level::Low, Speed::VeryHigh);
    let cs = Output::new(p.PE0, Level::High, Speed::VeryHigh);
    let ready = Input::new(p.PE1, Pull::Up);

    let mut wifi = EsWifiController::new(spi, cs, reset, wake, ready);
    match wifi.start().await {
        Ok(()) => defmt::info!("Started..."),
        Err(err) => defmt::info!("Error... {}", err),
    }


    let ip = wifi.join_wep(WIFI_SSID, WIFI_PSK).await;
    defmt::info!("Joined...");
    defmt::info!("IP {}", ip);


    // DEVICE.configure(MyDevice {
    //     //wifi: EsWifi::new(enable_pin, reset_pin),
    //     ticker: ActorContext::new(Ticker::new(Duration::from_millis(500), LedMessage::Toggle)),
    //     led: ActorContext::new(Led::new(Output::new(p.PA5, Level::High, Speed::Low))),
    // });

    // DEVICE
    //     .mount(|device| async move {
    //         // let mut wifi = device.wifi.mount((), spawner);
    //         // defmt::info!("wifi {} ", WIFI_SSID);
    //         // wifi.join(Join::Wpa {
    //         //     ssid: WIFI_SSID.trim_end(),
    //         //     password: WIFI_PSK.trim_end(),
    //         // })
    //         // .await
    //         // .expect("Error joining wifi");
    //         // defmt::info!("WiFi network joined");

    //         let led = device.led.mount((), spawner);
    //         let ticker = device.ticker.mount(led, spawner);
    //         ticker
    //     })
    //     .await;


        // let mut i =0;
        // loop {
        //     let mut buf = [0x0Au8; 4];
        //     unwrap!(cs.set_low());
        //     unwrap!(spi.transfer(&mut buf));
        //     unwrap!(cs.set_high());
        //     i = i + 1;
        //     info!("xfer {=[u8]:x} {}", buf, i);
        // }

}
