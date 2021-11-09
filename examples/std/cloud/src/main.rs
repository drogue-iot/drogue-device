#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{
    actors::net::*,
    actors::tcp::std::*,
    domain::{temperature::Temperature, SensorAcquisition},
    *,
};
use drogue_temperature::*;
use drogue_tls::Aes128GcmSha256;
use embassy::time::{Duration, Timer};
use rand::rngs::OsRng;

const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));
const HOST: &str = "http.sandbox.drogue.cloud";
const PORT: u16 = 443;

pub struct MyDevice {
    network: ActorContext<'static, StdTcpActor>,
    app: ActorContext<
        'static,
        App<TlsConnectionFactory<'static, StdTcpActor, Aes128GcmSha256, OsRng, 16384, 1>>,
    >,
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
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
    });

    let app = DEVICE
        .mount(|device| async move {
            let network = device.network.mount((), spawner);
            device
                .app
                .mount(TlsConnectionFactory::new(network, OsRng), spawner)
        })
        .await;

    loop {
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
        Timer::after(Duration::from_secs(5)).await;
    }
}
