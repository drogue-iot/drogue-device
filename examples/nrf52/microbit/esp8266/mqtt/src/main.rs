#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod rng;
use rng::*;

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{drivers::wifi::esp8266::Esp8266Socket, traits::button::Button};

use drogue_device::bsp::boards::nrf52::microbit::LedMatrix;
use drogue_device::{
    bsp::boards::nrf52::microbit::*, drivers::dns::*, drivers::wifi::esp8266::Esp8266Modem, *,
};
use ector::{actor, Actor, ActorContext, Address, Inbox};
use embassy_time::{Duration, Timer};
use embassy_util::Forever;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, TIMER0, UARTE0},
    uarte, Peripherals,
};
use embedded_tls::{Aes128GcmSha256, NoClock, TlsConfig, TlsConnection, TlsContext};

use embedded_nal_async::*;
use rust_mqtt::client::client_config::MqttVersion::MQTTv5;
use rust_mqtt::utils::rng_generator::CountingRng;
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::publish_packet::QualityOfService,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");
const HOST: &str = drogue::config!("hostname");
const PORT: &str = drogue::config!("port");
const USERNAME: &str = drogue::config!("mqtt-username");
const PASSWORD: &str = drogue::config!("mqtt-password");
const TOPIC: &str = drogue::config!("mqtt-topic");
const TOPIC_S: &str = drogue::config!("mqtt-command-topic");

type SERIAL = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    defmt::info!("Started");
    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 4096] = [0u8; 4096];
    static mut RX_BUFFER: [u8; 4096] = [0u8; 4096];
    let irq = interrupt::take!(UARTE0_UART0);
    static STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();
    let state = STATE.put(State::new());
    let uart = BufferedUarte::new(
        state,
        board.uarte0,
        board.timer0,
        board.ppi_ch0,
        board.ppi_ch1,
        irq,
        board.p15,
        board.p14,
        board.p1,
        board.p2,
        config,
        unsafe { &mut RX_BUFFER },
        unsafe { &mut TX_BUFFER },
    );

    let enable_pin = Output::new(board.p9, Level::High, OutputDrive::Standard);
    let reset_pin = Output::new(board.p8, Level::High, OutputDrive::Standard);

    let network = Esp8266Modem::new(uart, enable_pin, reset_pin);
    static NETWORK: Forever<Esp8266Modem<SERIAL, ENABLE, RESET, 2>> = Forever::new();
    let network = NETWORK.put(network);
    spawner
        .spawn(net_task(network, WIFI_SSID.trim_end(), WIFI_PSK.trim_end()))
        .unwrap();

    let ip = DNS
        .get_host_by_name(HOST, AddrType::IPv4)
        .await
        .expect("unable to resolve host");

    defmt::info!("Creating sockets");
    let addr = SocketAddr::new(ip, PORT.parse::<u16>().unwrap());

    let mut rng = Rng::new(nrf52833_pac::Peripherals::take().unwrap().RNG);

    let socket_pub = network.connect(addr).await.unwrap();
    let socket_sub = network.connect(addr).await.unwrap();

    static mut TLS_PUB_BUF: [u8; 16384] = [0; 16384];
    static mut TLS_SUB_BUF: [u8; 16384] = [0; 16384];
    let mut connection_pub: TlsConnection<'_, _, Aes128GcmSha256> =
        TlsConnection::new(socket_pub, unsafe { &mut TLS_PUB_BUF });
    let mut connection_recv: TlsConnection<'_, _, Aes128GcmSha256> =
        TlsConnection::new(socket_sub, unsafe { &mut TLS_SUB_BUF });

    let tls_config = TlsConfig::new().with_server_name(HOST);
    connection_pub
        .open::<_, NoClock, 1>(TlsContext::new(&tls_config, &mut rng))
        .await
        .unwrap();
    connection_recv
        .open::<_, NoClock, 1>(TlsContext::new(&tls_config, &mut rng))
        .await
        .unwrap();

    static RECEIVER: ActorContext<Receiver> = ActorContext::new();
    RECEIVER.mount(spawner, Receiver::new(board.display, connection_recv));

    let mut config = ClientConfig::new(MQTTv5, CountingRng(0));
    config.add_qos(QualityOfService::QoS0);
    config.add_username(USERNAME);
    config.add_password(PASSWORD);
    config.keep_alive = u16::MAX;
    let mut recv_buffer = [0; 1000];
    let mut write_buffer = [0; 1000];

    let mut client = MqttClient::<_, 20, CountingRng>::new(
        connection_pub,
        &mut write_buffer,
        1000,
        &mut recv_buffer,
        1000,
        config,
    );
    defmt::info!("[PUBLISHER] Connecting to broker");
    client.connect_to_broker().await.unwrap();
    let mut button = board.btn_a;
    loop {
        defmt::info!("[PUBLISHER] Press 'A' button to send data");
        button.wait_pressed().await;
        defmt::info!("[PUBLISHER] sending message");
        client.send_message(TOPIC, "{'temp':42}").await.unwrap();
        defmt::info!("[PUBLISHER] message sent");
    }
}

#[embassy_executor::task]
async fn net_task(
    modem: &'static Esp8266Modem<'static, SERIAL, ENABLE, RESET, 2>,
    ssid: &'static str,
    psk: &'static str,
) {
    loop {
        let _ = modem.run(ssid, psk).await;
    }
}

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
    DnsEntry::new(
        "mqtt.sandbox.drogue.cloud",
        IpAddr::V4(Ipv4Addr::new(65, 108, 135, 161)),
    ),
]);

type Connection = TlsConnection<'static, Esp8266Socket<'static, SERIAL>, Aes128GcmSha256>;
//type Connection = Esp8266Connection<'static, SERIAL>;

pub struct Receiver {
    display: LedMatrix,
    connection: Option<Connection>,
}

impl Receiver {
    pub fn new(display: LedMatrix, connection: Connection) -> Self {
        Self {
            display,
            connection: Some(connection),
        }
    }
}

#[derive(Clone)]
pub enum ReceiverMessage {
    Toggle,
}

#[actor]
impl Actor for Receiver {
    type Message<'m> = ReceiverMessage;

    async fn on_mount<M>(&mut self, _: Address<ReceiverMessage>, mut _inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        let mut config = ClientConfig::new(MQTTv5, CountingRng(0));
        config.add_qos(QualityOfService::QoS1);
        config.add_username(USERNAME);
        config.add_password(PASSWORD);
        config.keep_alive = u16::MAX;
        let mut recv_buffer = [0; 1000];
        let mut write_buffer = [0; 1000];

        let mut client = MqttClient::<Connection, 20, CountingRng>::new(
            self.connection.take().unwrap(),
            &mut write_buffer,
            1000,
            &mut recv_buffer,
            1000,
            config,
        );
        defmt::info!("[RECEIVER] Connecting to broker!");
        client.connect_to_broker().await.unwrap();
        defmt::info!("[RECEIVER] Subscribing to topic!");
        let _res = client.subscribe_to_topic(TOPIC_S).await;

        loop {
            defmt::info!("[RECEIVER] Waiting for new message");
            let msg = client.receive_message().await;
            if msg.is_ok() {
                let message = msg.unwrap();
                let act_message = core::str::from_utf8(message).unwrap();
                defmt::info!("[RECEIVER] Received: {}", act_message);
                self.display.scroll(act_message).await;
            } else {
                defmt::warn!("[RECEIVER] Could not get message!");
            }
            Timer::after(Duration::from_secs(2)).await;
        }
    }
}
