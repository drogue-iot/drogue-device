#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
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
use embassy::io::FromStdIo;
use embedded_hal::digital::v2::OutputPin;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;
use wifi_app::*;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use drogue_tls::{Aes128GcmSha256, TlsContext};
        use drogue_device::actors::socket::TlsSocket;
        use rand::rngs::OsRng;

        const HOST: &str = "http.sandbox.drogue.cloud";
        const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
        const PORT: u16 = 443;
        static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];
    } else {
        const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP for local network server
        const PORT: u16 = 12345;
    }
}

type UART = FromStdIo<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

#[cfg(feature = "tls")]
type AppSocket =
    TlsSocket<'static, Socket<'static, Esp8266Controller<'static>>, OsRng, Aes128GcmSha256>;

#[cfg(not(feature = "tls"))]
type AppSocket = Socket<'static, Esp8266Controller<'static>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<AppSocket>>,
}
static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = BufReader::new(port);
    let port = FromStdIo::new(port);

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
            log::info!("WiFi network joined");

            let socket = Socket::new(wifi, wifi.open().await);
            #[cfg(feature = "tls")]
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(OsRng, unsafe { &mut TLS_BUFFER })
                    .with_server_name(HOST.trim_end()),
            );
            device.app.mount(socket, spawner)
        })
        .await;

    app.request(Command::Send).unwrap().await;
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
