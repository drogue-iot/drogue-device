#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{
    actors::socket::*,
    actors::tcp::std::*,
    domain::{temperature::Temperature, SensorAcquisition},
    traits::ip::*,
    *,
};
use drogue_temperature::*;
use drogue_tls::{Aes128GcmSha256, TlsContext};
use rand::rngs::OsRng;

const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));
// TODO: Use these settings for Drogue Cloud sandbox:
// const HOST: &str = "http.sandbox.drogue.cloud";
// const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
// const PORT: u16 = 443;

const HOST: &str = "localhost";
const IP: IpAddress = IpAddress::new_v4(127, 0, 0, 1); // IP resolved for "http.sandbox.drogue.cloud"
const PORT: u16 = 8088;

static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];

type AppSocket = TlsSocket<'static, Socket<'static, StdTcpActor>, OsRng, Aes128GcmSha256>;

pub struct MyDevice {
    network: ActorContext<'static, StdTcpActor>,
    app: ActorContext<'static, App<AppSocket>>,
}
static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    DEVICE.configure(MyDevice {
        network: ActorContext::new(StdTcpActor::new()),
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
    });

    let app = DEVICE
        .mount(|device| async move {
            let network = device.network.mount((), spawner);

            let handle = network.open().await.unwrap();
            let socket = Socket::new(network, handle);
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(OsRng, unsafe { &mut TLS_BUFFER })
                    .with_server_name(HOST.trim_end()),
            );
            device.app.mount(socket, spawner)
        })
        .await;

    app.request(Command::Update(SensorData {
        data: SensorAcquisition {
            temperature: Temperature::new(22.0),
            relative_humidity: 0.0,
        },
        location: None,
    }))
    .unwrap()
    .await;

    app.request(Command::Send).unwrap().await;
}
