#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

mod serial;

use async_io::Async;
use drogue_device::{
    actors::{socket::*, wifi::esp8266::*},
    drivers::wifi::esp8266::*,
    traits::{ip::*, tcp::*, wifi::*},
    *,
};
use drogue_tls::{Aes128GcmSha256, TlsConfig};
use embassy::{io::FromStdIo, time, util::Forever};
use embedded_hal::digital::v2::OutputPin;
use futures::io::BufReader;
use nix::sys::termios;
use rand::rngs::OsRng;
use serial::*;
use wifi_app::*;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const HOST: &str = "http.sandbox.drogue.cloud";
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/drogue.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/drogue.password.txt"));

const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
const PORT: u16 = 443;

type UART = FromStdIo<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;
type AppSocket =
    TlsSocket<'static, Socket<'static, Esp8266Controller<'static>>, OsRng, Aes128GcmSha256, 16384>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<AppSocket>>,
}
static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

static TLS_CONFIG: Forever<TlsConfig<'static, OsRng, Aes128GcmSha256>> = Forever::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = BufReader::new(port);
    let port = FromStdIo::new(port);

    let tls_config = TLS_CONFIG.put(TlsConfig::new(OsRng).with_server_name(HOST.trim_end()));

    DEVICE.configure(MyDevice {
        wifi: Esp8266Wifi::new(port, DummyPin {}, DummyPin {}),
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
    });

    let app = DEVICE
        .mount(|device| async move {
            let mut wifi = device.wifi.mount((), spawner);
            wifi.join(Join::Wpa {
                ssid: WIFI_SSID.trim_end(),
                password: WIFI_PSK.trim_end(),
            })
            .await
            .expect("Error joining wifi");

            let socket = Socket::new(wifi, wifi.open().await);
            let tls_socket = TlsSocket::wrap(socket, tls_config);
            device.app.mount(tls_socket, spawner)
        })
        .await;

    loop {
        time::Timer::after(time::Duration::from_secs(20)).await;
        app.request(Command::Send).unwrap().await;
        break;
    }
}

pub struct DummyPin {}
impl OutputPin for DummyPin {
    type Error = ();
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
