#![macro_use]
#![feature(type_alias_impl_trait)]

mod serial;

use {
    async_io::Async,
    drogue_device::*,
    embassy_futures::select::{select, Either},
    embassy_time::{Duration, Timer},
    embedded_hal::digital::{ErrorType, OutputPin},
    embedded_io::adapters::FromFutures,
    esp8266_at_driver::*,
    futures::io::BufReader,
    nix::sys::termios,
    reqwless::{client::*, request::*},
    serial::*,
    static_cell::StaticCell,
};

#[path = "../../../common/dns.rs"]
mod dns;
use dns::*;

#[path = "../../../common/temperature.rs"]
mod temperature;
use temperature::*;

type SERIAL = FromFutures<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

/// HTTP endpoint hostname
const HOSTNAME: &str = drogue::config!("hostname");

/// HTTP endpoint port
const PORT: &str = drogue::config!("port");

/// HTTP username
const USERNAME: &str = drogue::config!("username");

/// HTTP password
const PASSWORD: &str = drogue::config!("password");

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = futures::io::BufReader::new(port);
    let port = FromFutures::new(port);

    let network = Esp8266Driver::new(port, DummyPin, DummyPin);
    static NETWORK: StaticCell<Esp8266Driver<SERIAL, ENABLE, RESET, 1>> = StaticCell::new();
    let network = NETWORK.init(network);
    spawner
        .spawn(net_task(network, WIFI_SSID.trim_end(), WIFI_PSK.trim_end()))
        .unwrap();

    let url = format!(
        "https://{}:{}/v1/temperature?data_schema=urn:drogue:iot:temperature",
        HOSTNAME, PORT
    );

    let mut tls = [0; 8000];
    let mut rng = rand::rngs::OsRng;
    let mut client = HttpClient::new_with_tls(network, &DNS, TlsConfig::new(&mut rng, &mut tls));

    loop {
        let sensor_data = TemperatureData {
            geoloc: None,
            temp: Some(22.2),
            hum: None,
        };

        match select(Timer::after(Duration::from_secs(20)), async {
            let tx: heapless::String<1024> = serde_json_core::ser::to_string(&sensor_data).unwrap();
            let mut rx_buf = [0; 1024];
            let response = client
                .request(Method::POST, &url)
                .await
                .unwrap()
                .basic_auth(USERNAME.trim_end(), PASSWORD.trim_end())
                .body(tx.as_bytes())
                .content_type(ContentType::ApplicationJson)
                .send(&mut rx_buf[..])
                .await;

            match response {
                Ok(response) => {
                    log::info!("Response status: {:?}", response.status);
                    if let Some(payload) = response.body {
                        let _s = core::str::from_utf8(payload).unwrap();
                    }
                }
                Err(e) => {
                    log::warn!("Error doing HTTP request: {:?}", e);
                }
            }
        })
        .await
        {
            Either::First(_) => {
                log::info!("Request timeout");
            }
            Either::Second(_) => {
                log::info!("Telemetry reported successfully");
            }
        }
        Timer::after(Duration::from_secs(10)).await;
    }
}

#[embassy_executor::task]
async fn net_task(
    modem: &'static Esp8266Driver<'static, SERIAL, ENABLE, RESET, 1>,
    ssid: &'static str,
    psk: &'static str,
) {
    loop {
        let _ = modem.run(ssid, psk).await;
    }
}

pub struct DummyPin;
impl ErrorType for DummyPin {
    type Error = ();
}

impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
