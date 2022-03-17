#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::socket::*,
    actors::wifi::*,
    bsp::{boards::nrf52::microbit::*, Board},
    clients::mqtt::*,
    domain::temperature::Celsius,
    drivers::dns::*,
    traits::dns::*,
    traits::ip::*,
    traits::tcp::*,
    *,
};
use drogue_device::{actors::wifi::esp8266::*, drogue, traits::wifi::*, DeviceContext, Package};
use embassy::util::Forever;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, UARTE0},
    uarte,
    uarte::{Uarte, UarteRx, UarteTx},
    Peripherals,
};
use rust_mqtt::{
    client::{client_config::ClientConfig, client_v5::MqttClientV5},
    packet::v5::{property::Property, publish_packet::QualityOfService},
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");
const HOST: &str = drogue::config!("hostname");
const PORT: &str = drogue::config!("port");
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

    let ips = DNS.resolve(HOST).await.expect("unable to resolve host");
    let ip = ips[0];
    let mut socket = Socket::new(wifi, wifi.open().await.unwrap());

    socket
        .connect(
            IpProtocol::Tcp,
            SocketAddress::new(ips[0], PORT.parse::<u16>().unwrap()),
        )
        .await
        .expect("Error creating connection");

    let mut config = ClientConfig::new();
    config.add_qos(QualityOfService::QoS1);
    config.add_username("xyz");
    config.add_password(PASSWORD);
    config.max_packet_size = 60;
    config.properties.push(Property::ReceiveMaximum(20));
    let mut recv_buffer = [0; 100];
    let mut write_buffer = [0; 100];

    let mut client = MqttClientV5::<_, 5>::new(
        DrogueNetwork::new(socket),
        &mut write_buffer,
        100,
        &mut recv_buffer,
        100,
        config,
    );

    client
        .connect
        .to_broker()
        .await
        .expect("error connecting to broker");
    client
        .send_message("foo", "Hello, World")
        .await
        .expect("error sending message");
}

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddress::new_v4(65, 108, 135, 161),
    ),
]);
