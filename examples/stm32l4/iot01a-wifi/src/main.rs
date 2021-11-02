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
    actors::button::*,
    actors::i2c::*,
    actors::sensors::hts221::*,
    actors::socket::*,
    actors::wifi::*,
    traits::{ip::*, wifi::*},
    *,
};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::rcc::{
    AHBPrescaler, ClockSrc, MSIRange, PLLClkDiv, PLLMul, PLLSource, PLLSrcDiv,
};
use embassy_stm32::spi::{self, Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_stm32::{
    exti::*,
    gpio::{Input, Level, Output, Pull, Speed},
    i2c, interrupt,
    peripherals::{
        DMA1_CH4, DMA1_CH5, DMA2_CH1, DMA2_CH2, I2C2, PB13, PC13, PD15, PE0, PE1, PE8, SPI3,
    },
    rcc::Rcc,
    Config, Peripherals,
};
use wifi_app::*;
//use defmt::*;
//use embedded_hal::digital::v2::{InputPin, OutputPin};

use drogue_device::drivers::wifi::eswifi::EsWifiController;

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use embassy_stm32::{
            rng::Rng,
            peripherals::RNG,
        };
        use drogue_tls::{Aes128GcmSha256, TlsContext};
        use drogue_device::actors::socket::TlsSocket;

        //const HOST: &str = "http.sandbox.drogue.cloud";
        //const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
        //const PORT: u16 = 443;

        const HOST: &str = "localhost";
        const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP of local drogue service
        const PORT: u16 = 8088;
        static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];
    } else {
        const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP for local network server
        const PORT: u16 = 8088;
    }
}

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

type WAKE = Output<'static, PB13>;
type RESET = Output<'static, PE8>;
type CS = Output<'static, PE0>;
type READY = ExtiInput<'static, PE1>;
type SPI = Spi<'static, SPI3, DMA2_CH2, DMA2_CH1>;
type SpiError = spi::Error;

type EsWifi = EsWifiController<SPI, CS, RESET, WAKE, READY, SpiError>;

#[cfg(feature = "tls")]
type AppSocket =
    TlsSocket<'static, Socket<'static, AdapterActor<EsWifi>>, Rng<RNG>, Aes128GcmSha256>;

#[cfg(not(feature = "tls"))]
type AppSocket = Socket<'static, AdapterActor<EsWifi>>;

type I2cDriver = embassy_stm32::i2c::I2c<'static, I2C2, DMA1_CH4, DMA1_CH5>;

pub struct MyDevice {
    wifi: ActorContext<'static, AdapterActor<EsWifi>>,
    app: ActorContext<'static, App<AppSocket>, 2>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App<AppSocket>>>,
    i2c: ActorContext<'static, I2cPeripheral<I2cDriver>>,
    sensor: ActorContext<
        'static,
        Sensor<
            ExtiInput<'static, PD15>,
            Address<'static, I2cPeripheral<I2cDriver>>,
            AppAddress<AppSocket>,
        >,
    >,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

// Clock configuration that enables suffiently fast clock for RNG
fn config() -> Config {
    let mut config = Config::default();
    config.rcc = config
        .rcc
        .clock_src(ClockSrc::PLL(
            PLLSource::HSI16,
            PLLClkDiv::Div2,
            PLLSrcDiv::Div1,
            PLLMul::Mul10,
            Some(PLLClkDiv::Div2),
        ))
        .ahb_pre(AHBPrescaler::Div8);
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    /*
    let rcc = Rcc::new(p.RCC);
    defmt::info!(
        "Starting up... with system clock: {} Hz",
        rcc.clocks().sys.0
    );
    */

    let spi = Spi::new(
        p.SPI3,
        p.PC10,
        p.PC12,
        p.PC11,
        p.DMA2_CH2,
        p.DMA2_CH1,
        Hertz(1_000_000),
        SpiConfig::default(),
    );

    let _boot = Output::new(p.PB12, Level::Low, Speed::VeryHigh);
    let wake = Output::new(p.PB13, Level::Low, Speed::VeryHigh);
    let reset = Output::new(p.PE8, Level::Low, Speed::VeryHigh);
    let cs = Output::new(p.PE0, Level::High, Speed::VeryHigh);
    let ready = Input::new(p.PE1, Pull::Up);
    let ready = ExtiInput::new(ready, p.EXTI1);

    let mut wifi = EsWifiController::new(spi, cs, reset, wake, ready);
    match wifi.start().await {
        Ok(()) => defmt::info!("Started..."),
        Err(err) => defmt::info!("Error... {}", err),
    }

    let button_pin = Input::new(p.PC13, Pull::Up);
    let button = ExtiInput::new(button_pin, p.EXTI13);

    let ready_pin = Input::new(p.PD15, Pull::Down);
    let sensor_ready = ExtiInput::new(ready_pin, p.EXTI15);

    let i2c_irq = interrupt::take!(I2C2_EV);
    let i2c = i2c::I2c::new(
        p.I2C2,
        p.PB10,
        p.PB11,
        i2c_irq,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Hertz(100_000),
    );

    /*
    let ip = wifi.join_wep(WIFI_SSID, WIFI_PSK).await;
    defmt::info!("Joined...");
    defmt::info!("IP {}", ip);
    */

    #[cfg(feature = "tls")]
    let rng = Rng::new(p.RNG);

    DEVICE.configure(MyDevice {
        wifi: ActorContext::new(AdapterActor::new()),
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
        button: ActorContext::new(Button::new(button)),
        i2c: ActorContext::new(I2cPeripheral::new(i2c)),
        sensor: ActorContext::new(Sensor::new(sensor_ready)),
    });

    DEVICE
        .mount(|device| async move {
            let mut wifi = device.wifi.mount(wifi, spawner);
            defmt::info!("Joining WiFi network...");
            wifi.join(Join::Wpa {
                ssid: WIFI_SSID.trim_end(),
                password: WIFI_PSK.trim_end(),
            })
            .await
            .expect("Error joining wifi");
            defmt::info!("WiFi network joined");

            let socket = Socket::new(wifi, wifi.open().await.unwrap());
            #[cfg(feature = "tls")]
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(rng, unsafe { &mut TLS_BUFFER }).with_server_name(HOST.trim_end()),
            );

            let app = device.app.mount(socket, spawner);
            device.button.mount(app, spawner);
            let i2c = device.i2c.mount((), spawner);
            device.sensor.mount((i2c, app.into()), spawner);
        })
        .await;
    defmt::info!("Application initialized. Press 'User' button to send data");
}
