#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use drogue_device::drivers::sensors::hts221::Hts221;
use drogue_device::{
    bsp::{boards::stm32l4::iot01a::*, Board},
    domain::temperature::Celsius,
    traits::wifi::*,
    *,
};
use drogue_device::{
    drivers::dns::{DnsEntry, StaticDnsResolver},
    drogue,
};
use drogue_temperature::*;
use embassy_time::Duration;
use embassy_util::Forever;
use embassy_stm32::{flash::Flash, Peripherals};
use embedded_nal_async::{AddrType, Dns, IpAddr, Ipv4Addr, SocketAddr, TcpConnect};

#[cfg(feature = "dfu")]
use drogue_device::firmware::FirmwareManager;

#[cfg(feature = "dfu")]
use embedded_update::{service::DrogueHttp, DeviceStatus};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

bind_bsp!(Iot01a, BSP);

impl TemperatureBoard for BSP {
    type Network = &'static SharedEsWifi;
    type TemperatureScale = Celsius;
    type SendTrigger = TimeTrigger;
    type Sensor = Hts221<I2c2>;
    type SensorReadyIndicator = Hts221Ready;
    type Rng = TlsRand;
}

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

static DEVICE: Forever<TemperatureDevice<BSP>> = Forever::new();

#[embassy_executor::main(config = "Iot01a::config(true)")]
async fn main(spawner: embassy_executor::Spawner, p: Peripherals) {
    let board = Iot01a::new(p);
    unsafe {
        RNG_INST.replace(board.rng);
    }

    static NETWORK: Forever<SharedEsWifi> = Forever::new();
    let network: &'static SharedEsWifi = NETWORK.put(SharedEsWifi::new(board.wifi));
    #[cfg(feature = "dfu")]
    {
        spawner.spawn(updater_task(network, board.flash)).unwrap();
    }

    spawner
        .spawn(network_task(
            network,
            WIFI_SSID.trim_end(),
            WIFI_PSK.trim_end(),
        ))
        .unwrap();

    let device = DEVICE.put(TemperatureDevice::new());
    let config = TemperatureBoardConfig {
        send_trigger: TimeTrigger(Duration::from_secs(60)),
        sensor_ready: board.hts221_ready,
        sensor: Hts221::new(board.i2c2),
        network,
    };
    device.mount(spawner, TlsRand, config).await;

    defmt::info!("Application running");
}

#[embassy_executor::task]
async fn network_task(adapter: &'static SharedEsWifi, ssid: &'static str, psk: &'static str) {
    loop {
        let _ = adapter.run(ssid, psk).await;
    }
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

#[cfg(feature = "dfu")]
#[embassy_executor::task]
async fn updater_task(network: &'static SharedEsWifi, flash: Flash<'static>) {
    use drogue_device::firmware::BlockingFlash;
    use embassy_time::{Delay, Timer};

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);
    let updater = embassy_boot_stm32::FirmwareUpdater::default();

    let ip = DNS
        .get_host_by_name(HOST.trim_end(), AddrType::IPv4)
        .await
        .unwrap();

    let service: DrogueHttp<'_, _, _, 2048> = DrogueHttp::new(
        network,
        TlsRand,
        SocketAddr::new(ip, PORT.parse::<u16>().unwrap()),
        HOST.trim_end(),
        USERNAME.trim_end(),
        PASSWORD.trim_end(),
    );

    let mut device: FirmwareManager<BlockingFlash<Flash<'static>>, 4096, 2048> =
        FirmwareManager::new(BlockingFlash::new(flash), updater, version.as_bytes());
    let mut updater = embedded_update::FirmwareUpdater::new(
        service,
        embedded_update::UpdaterConfig {
            timeout_ms: 40_000,
            backoff_ms: 100,
        },
    );
    loop {
        defmt::info!("Starting updater task");
        match updater.run(&mut device, &mut Delay).await {
            Ok(s) => {
                defmt::info!("Updater finished with status: {:?}", s);
                match s {
                    DeviceStatus::Updated => {
                        defmt::debug!("Resetting device");
                        cortex_m::peripheral::SCB::sys_reset();
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
            Err(e) => {
                defmt::warn!("Error running updater: {:?}", defmt::Debug2Format(&e));
                Timer::after(Duration::from_secs(10)).await;
            }
        }
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
