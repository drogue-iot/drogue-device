#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod rng;
use rng::*;

use defmt_rtt as _;
use drogue_tls::Aes128GcmSha256;
use panic_probe as _;

use drogue_device::traits::button::Button;

use drogue_device::bsp::boards::nrf52::microbit::LedMatrix;
use drogue_device::drivers::wifi::esp8266::*;
use drogue_device::{
    bsp::boards::nrf52::microbit::*, drivers::dns::*, drivers::wifi::esp8266::Esp8266Controller,
    traits::ip::*, *,
};
use embassy::time::{Duration, Timer};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, UARTE0},
    uarte,
    uarte::{Uarte, UarteRx, UarteTx},
    Peripherals,
};

use drogue_device::network::connection::{
    ConnectionFactory, TlsConnectionFactory, TlsNetworkConnection,
};
use drogue_device::network::tcp::TcpStackState;
use drogue_device::shared::Handle;
use drogue_device::traits::dns::DnsResolver;
use drogue_device::traits::wifi::{Join, WifiSupplicant};
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

type TX = UarteTx<'static, UARTE0>;
type RX = UarteRx<'static, UARTE0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    defmt::info!("Started");
    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let irq = interrupt::take!(UARTE0_UART0);
    let uart = Uarte::new_with_rtscts(
        board.uarte0,
        irq,
        board.p15,
        board.p14,
        board.p1,
        board.p2,
        config,
    );

    let (tx, rx) = uart.split();

    let enable_pin = Output::new(board.p9, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(board.p8, Level::Low, OutputDrive::Standard);

    static WIFI: Esp8266Driver = Esp8266Driver::new();
    let (mut network, modem) = WIFI.initialize(tx, rx, enable_pin, reset_pin);
    spawner.spawn(wifi(modem)).unwrap();

    network
        .join(Join::Wpa {
            ssid: WIFI_SSID.trim_end(),
            password: WIFI_PSK.trim_end(),
        })
        .await
        .expect("Error joining WiFi network");

    static NETWORK: TcpStackState<Esp8266Controller<'static, TX>> = TcpStackState::new();
    let network = NETWORK.initialize(network);

    let mut conn_factory = {
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
        static mut TLS_BUFFER_SEC: [u8; 16384] = [0; 16384];
        TlsConnectionFactory::<'static, _, Aes128GcmSha256, _, 2>::new(
            network.clone(),
            Rng::new(nrf52833_pac::Peripherals::take().unwrap().RNG),
            unsafe { [&mut TLS_BUFFER, &mut TLS_BUFFER_SEC] },
        )
    };

    let ips = DNS.resolve(HOST).await.expect("unable to resolve host");

    defmt::info!("Creating sockets");

    let connection_pub = conn_factory
        .connect(HOST, ips[0], PORT.parse::<u16>().unwrap())
        .await
        .unwrap();
    let connection_recv = conn_factory
        .connect(HOST, ips[0], PORT.parse::<u16>().unwrap())
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

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "mqtt.sandbox.drogue.cloud",
        IpAddress::new_v4(65, 108, 135, 161),
    ),
]);

#[embassy::task]
pub async fn wifi(mut modem: Esp8266Modem<'static, RX, ENABLE, RESET>) {
    modem.run().await;
}

pub struct Receiver {
    display: LedMatrix,
    connection: Option<
        TlsNetworkConnection<
            'static,
            Handle<'static, Esp8266Controller<'static, UarteTx<'static, UARTE0>>>,
            Aes128GcmSha256,
        >,
    >,
}

impl Receiver {
    pub fn new(
        display: LedMatrix,
        connection: TlsNetworkConnection<
            'static,
            Handle<'static, Esp8266Controller<'static, UarteTx<'static, UARTE0>>>,
            Aes128GcmSha256,
        >,
    ) -> Self {
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

        let mut client = MqttClient::<
            TlsNetworkConnection<
                '_,
                Handle<'_, Esp8266Controller<'static, UarteTx<'static, UARTE0>>>,
                Aes128GcmSha256,
            >,
            20,
            CountingRng,
        >::new(
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
