//! Over the air updates using Drogue Cloud
use {
    core::fmt::Write,
    embassy_time::{Delay, Duration, Timer},
    embedded_nal_async::{Dns, TcpConnect},
    embedded_update::{DeviceStatus, FirmwareDevice},
    heapless::String,
    http::HttpUpdater,
    rand_core::{CryptoRng, RngCore},
    reqwless::client::TlsConfig,
};

mod http;
pub mod lorawan;

/// Configuration for an OTA task
pub struct OtaConfig<'a> {
    pub hostname: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
}

/// Async task checking for Over The Air updates from Drogue Cloud and applying
pub async fn ota_task<TCP, DNS, DEVICE, RNG, RESET>(
    network: TCP,
    dns: &DNS,
    mut device: DEVICE,
    mut rng: RNG,
    config: OtaConfig<'_>,
    reset: RESET,
) where
    TCP: TcpConnect,
    DNS: Dns,
    DEVICE: FirmwareDevice,
    RNG: RngCore + CryptoRng,
    RESET: FnOnce(),
{
    let mut tls_buffer: [u8; 6000] = [0; 6000];
    let tls = TlsConfig::new(&mut rng, &mut tls_buffer);

    let mut url: String<64> = String::new();
    let _ = write!(
        url,
        "https://{}:{}/v1/dfu?ct=30",
        config.hostname, config.port
    );

    let service: HttpUpdater<'_, _, _, TlsConfig<'_, RNG>, 2048> = HttpUpdater::new(
        &network,
        dns,
        tls,
        url.as_str(),
        config.username,
        config.password,
    );

    let mut updater = embedded_update::FirmwareUpdater::new(
        service,
        embedded_update::UpdaterConfig {
            timeout_ms: 40_000,
            backoff_ms: 100,
        },
    );
    loop {
        info!("Starting updater task");
        match updater.run(&mut device, &mut Delay).await {
            Ok(s) => {
                info!("Updater finished with status: {:?}", s);
                match s {
                    DeviceStatus::Updated => {
                        debug!("Resetting device");
                        reset();
                        return;
                    }
                    DeviceStatus::Synced(delay) => {
                        if let Some(delay) = delay {
                            Timer::after(Duration::from_secs(delay as u64)).await;
                        } else {
                            Timer::after(Duration::from_secs(10)).await;
                        }
                    }
                }
            }
            Err(_e) => {
                warn!("Error running updater");
                Timer::after(Duration::from_secs(10)).await;
            }
        }
    }
}
