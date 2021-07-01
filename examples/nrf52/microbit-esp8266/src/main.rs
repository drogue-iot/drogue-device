#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

mod rng;
use rng::*;
use wifi_app::*;

use log::LevelFilter;
use panic_probe as _;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use drogue_device::{
    actors::{
        button::Button,
        socket::{Socket, TlsSocket},
        wifi::esp8266::*,
    },
    drivers::wifi::esp8266::Esp8266Controller,
    traits::{ip::*, tcp::TcpStack, wifi::*},
    ActorContext, DeviceContext, Package,
};
use drogue_tls::{Aes128GcmSha256, TlsContext};
use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    gpio::{Input, Level, NoPin, Output, OutputDrive, Pull},
    gpiote::PortInput,
    interrupt,
    peripherals::{P0_09, P0_10, P0_14, TIMER0, UARTE0},
    uarte, Peripherals,
};
use nrf52833_pac as pac;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));

const HOST: &str = "http.sandbox.drogue.cloud";
const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
const PORT: u16 = 443;

// const HOST: &str = "example.com";
// const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP resolved for "http.sandbox.drogue.cloud"
// const PORT: u16 = 12345;

const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/drogue.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/drogue.password.txt"));

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;
type AppSocket =
    TlsSocket<'static, Socket<'static, Esp8266Controller<'static>>, Rng, Aes128GcmSha256>;
//type AppSocket = Socket<'static, Esp8266Controller<'static>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<AppSocket>>,
    button: ActorContext<'static, Button<'static, PortInput<'static, P0_14>, App<AppSocket>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    rtt_init_print!();
    log::set_logger(&LOGGER).unwrap();

    log::set_max_level(log::LevelFilter::Info);

    let button_port = PortInput::new(Input::new(p.P0_14, Pull::Up));

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 8192] = [0u8; 8192];
    static mut RX_BUFFER: [u8; 8192] = [0u8; 8192];

    let irq = interrupt::take!(UARTE0_UART0);
    let u = unsafe {
        BufferedUarte::new(
            p.UARTE0,
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            irq,
            p.P0_13,
            p.P0_01,
            NoPin,
            NoPin,
            config,
            &mut RX_BUFFER,
            &mut TX_BUFFER,
        )
    };

    let enable_pin = Output::new(p.P0_09, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(p.P0_10, Level::Low, OutputDrive::Standard);

    let rng = Rng::new(pac::Peripherals::take().unwrap().RNG);

    DEVICE.configure(MyDevice {
        wifi: Esp8266Wifi::new(u, enable_pin, reset_pin),
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
        button: ActorContext::new(Button::new(button_port)),
    });

    static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];

    DEVICE
        .mount(|device| async move {
            let mut wifi = device.wifi.mount((), spawner);
            wifi.join(Join::Wpa {
                ssid: WIFI_SSID.trim_end(),
                password: WIFI_PSK.trim_end(),
            })
            .await
            .expect("Error joining wifi");
            log::info!("WiFi network joined");

            let socket = Socket::new(wifi, wifi.open().await);
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(rng, unsafe { &mut TLS_BUFFER }).with_server_name(HOST.trim_end()),
            );
            let app = device.app.mount(socket, spawner);
            device.button.mount(app, spawner);
        })
        .await;
    log::info!("Application initialized. Press 'A' button to send data");
}
