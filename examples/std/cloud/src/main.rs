#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {
    async_io::Async,
    drogue_device::*,
    embassy_futures::select::{select, Either},
    embassy_time::{Duration, Timer},
    embedded_io::adapters::FromFutures,
    embedded_nal_async::*,
    futures::io::BufReader,
    rand::RngCore,
    reqwless::{client::*, request::*},
    std::net::TcpStream,
};

#[path = "../../../common/dns.rs"]
mod dns;
use dns::*;

#[path = "../../../common/temperature.rs"]
mod temperature;
use temperature::*;

/// HTTP endpoint hostname
const HOSTNAME: &str = drogue::config!("hostname");

/// HTTP endpoint port
const PORT: &str = drogue::config!("port");

/// HTTP username
const USERNAME: &str = drogue::config!("username");

/// HTTP password
const PASSWORD: &str = drogue::config!("password");

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let url = format!(
        "https://{}:{}/v1/temperature?data_schema=urn:drogue:iot:temperature",
        HOSTNAME, PORT
    );

    let mut tls = [0; 16384];
    let mut rng = rand::rngs::OsRng;
    let mut client =
        HttpClient::new_with_tls(&TcpClient, &DNS, TlsConfig::new(rng.next_u64(), &mut tls));

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
            Either::Second(_) => {}
        }
        Timer::after(Duration::from_secs(10)).await;
    }
}

pub struct TcpClient;

impl TcpConnect for TcpClient {
    type Error = std::io::Error;
    type Connection<'m> = FromFutures<BufReader<Async<TcpStream>>>;
    async fn connect<'m>(&'m self, remote: SocketAddr) -> Result<Self::Connection<'m>, Self::Error>
    where
        Self: 'm,
    {
        match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
            Ok(stream) => {
                let stream = Async::new(stream).unwrap();
                let stream = futures::io::BufReader::new(stream);
                let stream = FromFutures::new(stream);
                Ok(stream)
            }
            Err(e) => Err(e),
        }
    }
}
