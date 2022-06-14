#![no_std]
#![macro_use]

mod fmt;

use drogue_device::{
    drivers::dns::{DnsEntry, StaticDnsResolver},
    drogue,
    firmware::{remote::DrogueHttpUpdateService, FirmwareManager},
};
use embassy::time::{Delay, Duration, Timer};
use embassy_boot::FirmwareUpdater;
use embassy_embedded_hal::adapter::BlockingAsync;
use embedded_nal_async::{AddrType, Dns, IpAddr, Ipv4Addr, SocketAddr, TcpClient};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use rand_core::{CryptoRng, RngCore};

pub async fn run_updater<T: TcpClient, RNG: CryptoRng + RngCore, F: NorFlash + ReadNorFlash, const PAGE_SIZE: usize, const MTU: usize>(
    version: &'static [u8],
    updater: FirmwareUpdater,
    network: T,
    rng: RNG,
    flash: F,
) {
    let ip = DNS
        .get_host_by_name(HOST.trim_end(), AddrType::IPv4)
        .await
        .unwrap();

    let service: DrogueHttpUpdateService<'_, _, _, MTU> = DrogueHttpUpdateService::new(
        network,
        rng,
        SocketAddr::new(ip, PORT.parse::<u16>().unwrap()),
        HOST.trim_end(),
        USERNAME.trim_end(),
        PASSWORD.trim_end(),
    );

    let mut device: FirmwareManager<FirmwareConfig<F>, PAGE_SIZE, MTU> =
        FirmwareManager::new(FirmwareConfig::new(flash), updater, version);
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
            }
            Err(e) => {
                warn!("Error running updater: {:?}", e);
            }
        }
        Timer::after(Duration::from_secs(10)).await;
    }
}

pub struct FirmwareConfig<F: NorFlash + ReadNorFlash> {
    flash: BlockingAsync<F>,
}

impl<F: NorFlash + ReadNorFlash> FirmwareConfig<F> {
    pub fn new(flash: F) -> Self {
        Self {
            flash: BlockingAsync::new(flash),
        }
    }
}

impl<F: NorFlash + ReadNorFlash> drogue_device::firmware::FirmwareConfig for FirmwareConfig<F> {
    type STATE = BlockingAsync<F>;
    type DFU = BlockingAsync<F>;
    const BLOCK_SIZE: usize = F::ERASE_SIZE;

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.flash
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.flash
    }
}

const HOST: &str = drogue::config!("hostname");
const PORT: &str = drogue::config!("port");
const USERNAME: &str = drogue::config!("http-username");
const PASSWORD: &str = drogue::config!("http-password");

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddr::V4(Ipv4Addr::new(65, 108, 135, 161)),
    ),
]);
