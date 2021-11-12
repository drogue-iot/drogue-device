#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod serial;

use async_io::Async;
use drogue_device::{
    actors::wifi::{esp8266::*, AdapterActor},
    drivers::wifi::esp8266::*,
    traits::wifi::*,
    *,
};
use drogue_temperature::*;
use embassy::io::FromStdIo;
use embedded_hal::digital::v2::OutputPin;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use rand::rngs::OsRng;
        use drogue_tls::{Aes128GcmSha256};
        use drogue_device::actors::net::TlsConnectionFactory;

        const HOST: &str = "http.sandbox.drogue.cloud";
        const PORT: u16 = 443;
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
    } else {
        use drogue_device::Address;

        const HOST: &str = "localhost";
        const PORT: u16 = 8088;
    }
}

type UART = FromStdIo<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

#[cfg(feature = "tls")]
type ConnectionFactory = TlsConnectionFactory<
    'static,
    AdapterActor<Esp8266Controller<'static>>,
    Aes128GcmSha256,
    OsRng,
    1,
>;

#[cfg(not(feature = "tls"))]
type ConnectionFactory = Address<'static, AdapterActor<Esp8266Controller<'static>>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<ConnectionFactory>>,
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
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
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

            let factory = wifi;
            #[cfg(feature = "tls")]
            let factory =
                TlsConnectionFactory::new(factory, OsRng, [unsafe { &mut TLS_BUFFER }; 1]);

            device.app.mount(factory, spawner)
        })
        .await;

    app.request(Command::Update(TemperatureData {
        temp: Some(22.0),
        hum: None,
        geoloc: None,
    }))
    .unwrap()
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
