#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{actors::wifi::esp8266::*, drogue, traits::wifi::*, DeviceContext, Package};
use drogue_device::{
    actors::wifi::*,
    bsp::{boards::nrf52::microbit::*, Board},
    domain::temperature::Celsius,
    *,
};
use embassy::util::Forever;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, UARTE0},
    uarte,
    uarte::{Uarte, UarteRx, UarteTx},
    Peripherals,
};
use rust_mqtt::*;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");
const USERNAME: &str = drogue::config!("mqtt-username");
const PASSWORD: &str = drogue::config!("mqtt-password");
const TOPIC: &str = drogue::config!("mqtt-topic");

type TX = UarteTx<'static, UARTE0>;
type RX = UarteRx<'static, UARTE0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

bind_bsp!(Microbit, BSP);

type WifiDriver = Esp8266Wifi<TX, RX, ENABLE, RESET>;
type WifiActor = <WifiDriver as Package>::Primary;

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let irq = interrupt::take!(UARTE0_UART0);
    let uart = Uarte::new_with_rtscts(
        board.uarte0,
        irq,
        board.p0_13,
        board.p0_01,
        board.p0_03,
        board.p0_04,
        config,
    );

    let (tx, rx) = uart.split();

    let enable_pin = Output::new(board.p0_09, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(board.p0_10, Level::Low, OutputDrive::Standard);

    static DRIVER: Forever<WifiDriver> = Forever::new();
    let driver = DRIVER.put(Esp8266Wifi::new(tx, rx, enable_pin, reset_pin));

    let mut wifi = driver.mount((), spawner);
    wifi.join(Join::Wpa {
        ssid: WIFI_SSID.trim_end(),
        password: WIFI_PSK.trim_end(),
    })
    .await
    .unwrap();

    let mut config = ClientConfig::new();
    config.add_qos(qos);
    config.add_username("xyz");
    config.add_password(PASSWORD);
    config.max_packet_size = 60;
    config.properties.push(Property::ReceiveMaximum(20));
    let mut recv_buffer = [0; 100];
    let mut write_buffer = [0; 100];

    let mut client =
        MqttClientV5::<_, 5>::new(wifi, &mut write_buffer, 100, &mut recv_buffer, 100, config);
}
