#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use {
    core::fmt::Write,
    disco_iot01a::*,
    drogue_device::{
        drogue,
        firmware::FirmwareManager,
        ota::{ota_task, OtaConfig},
        *,
    },
    embassy_futures::select::{select, Either},
    embassy_stm32::flash::Flash,
    embassy_time::{Duration, Timer},
    embedded_io::ErrorKind,
    embedded_nal_async::{AddrType, Dns, IpAddr, Ipv4Addr, SocketAddr, TcpConnect},
    embedded_update::{service::DrogueHttp, DeviceStatus},
    heapless::String,
    hts221_async::*,
    reqwless::{
        client::{HttpClient, TlsConfig},
        request::{ContentType, Method, Response},
    },
    static_cell::StaticCell,
};

#[path = "../../../../common/dns.rs"]
mod dns;
use dns::*;

#[path = "../../../../common/temperature.rs"]
mod temperature;
use temperature::*;

use defmt_rtt as _;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

/// WiFi configuration settings
const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

/// Firmware version and revision override
const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

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
    let board = DiscoIot01a::default();
    // Create a globally shared random generator
    unsafe {
        RNG_INST.replace(board.rng);
    }

    static NETWORK: StaticCell<EsWifi> = StaticCell::new();
    let network: &'static EsWifi = NETWORK.init(board.wifi);

    // Start driver
    spawner
        .spawn(network_task(
            network,
            WIFI_SSID.trim_end(),
            WIFI_PSK.trim_end(),
        ))
        .unwrap();

    // Launch updater task
    spawner.spawn(updater_task(network, board.flash)).unwrap();

    let mut sensor = Hts221::new(board.i2c2);
    sensor.initialize().await.ok().unwrap();
    let mut ready = board.hts221_ready;

    let mut url: String<128> = String::new();
    write!(
        url,
        "https://{}:{}/v1/temperature?data_schema=urn:drogue:iot:temperature",
        HOSTNAME, PORT
    )
    .unwrap();

    let mut tls = [0; 8000];
    let mut rng = TlsRand;
    let mut client =
        HttpClient::new_with_tls(network, &dns::DNS, TlsConfig::new(&mut rng, &mut tls));

    loop {
        // Wait until we have a sensor reading
        while !ready.is_high() {
            ready.wait_for_any_edge().await;
        }

        let Ok(data) = sensor.read().await else {
            continue;
        };

        defmt::info!("Read sensor value: {:?}", data);

        let sensor_data = TemperatureData {
            geoloc: None,
            temp: Some(data.temperature.raw_value()),
            hum: Some(data.relative_humidity),
        };

        match select(Timer::after(Duration::from_secs(20)), async {
            let tx: String<128> = serde_json_core::ser::to_string(&sensor_data).unwrap();
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
                    defmt::info!("Response status: {:?}", response.status);
                    if let Some(payload) = response.body {
                        let _s = core::str::from_utf8(payload).unwrap();
                    }
                }
                Err(e) => {
                    defmt::warn!("Error doing HTTP request: {:?}", e);
                }
            }
        })
        .await
        {
            Either::First(_) => {
                defmt::info!("Request timeout");
            }
            Either::Second(_) => {
                defmt::info!("Telemetry reported successfully");
            }
        }
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[embassy_executor::task]
async fn network_task(adapter: &'static EsWifi, ssid: &'static str, psk: &'static str) {
    loop {
        let _ = adapter.run(ssid, psk).await;
    }
}

#[embassy_executor::task]
async fn updater_task(network: &'static EsWifi, flash: Flash<'static>) {
    use {
        drogue_device::firmware::BlockingFlash,
        embassy_time::{Delay, Timer},
    };

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);
    let updater = embassy_boot_stm32::FirmwareUpdater::default();

    let device: FirmwareManager<BlockingFlash<Flash<'static>>, 4, 2048> =
        FirmwareManager::new(BlockingFlash::new(flash), updater, version.as_bytes());

    let config = OtaConfig {
        hostname: HOSTNAME.trim_end(),
        port: PORT.parse::<u16>().unwrap(),
        username: USERNAME.trim_end(),
        password: PASSWORD.trim_end(),
    };

    Timer::after(Duration::from_secs(5)).await;
    ota_task(network, &DNS, device, TlsRand, config, || {
        cortex_m::peripheral::SCB::sys_reset()
    })
    .await
}

static mut RNG_INST: Option<Rng> = None;

#[no_mangle]
fn _embassy_rand(buf: &mut [u8]) {
    use rand_core::RngCore;

    critical_section::with(|_| unsafe {
        defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
    });
}

pub struct TlsRand;

impl rand_core::RngCore for TlsRand {
    fn next_u32(&mut self) -> u32 {
        critical_section::with(|_| unsafe { defmt::unwrap!(RNG_INST.as_mut()).next_u32() })
    }
    fn next_u64(&mut self) -> u64 {
        critical_section::with(|_| unsafe { defmt::unwrap!(RNG_INST.as_mut()).next_u64() })
    }
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        critical_section::with(|_| unsafe {
            defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
        });
    }
    fn try_fill_bytes(&mut self, buf: &mut [u8]) -> Result<(), rand_core::Error> {
        critical_section::with(|_| unsafe {
            defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
        });
        Ok(())
    }
}
impl rand_core::CryptoRng for TlsRand {}
