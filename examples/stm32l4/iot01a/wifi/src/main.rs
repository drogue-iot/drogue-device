#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
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
use drogue_temperature::*;
use embassy::time::Duration;
use embassy::util::Forever;
use embassy_stm32::{flash::Flash, Peripherals};

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
    type Network = EsWifiClient;
    type TemperatureScale = Celsius;
    type SendTrigger = TimeTrigger;
    type Sensor = Hts221<I2c2>;
    type SensorReadyIndicator = Hts221Ready;
    type Rng = TlsRand;
}

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

static DEVICE: Forever<TemperatureDevice<BSP>> = Forever::new();

#[embassy::main(config = "Iot01a::config(true)")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Iot01a::new(p);
    let mut wifi = board.wifi;

    match wifi.start().await {
        Ok(()) => defmt::info!("Started..."),
        Err(err) => defmt::info!("Error... {}", defmt::Debug2Format(&err)),
    }

    defmt::info!("Joining WiFi network...");
    wifi.join(Join::Wpa {
        ssid: WIFI_SSID.trim_end(),
        password: WIFI_PSK.trim_end(),
    })
    .await
    .expect("Error joining wifi");
    defmt::info!("WiFi network joined");

    unsafe {
        RNG_INST.replace(board.rng);
    }

    static NETWORK: Forever<SharedEsWifi> = Forever::new();
    let network = NETWORK.put(SharedEsWifi::new(wifi));
    let client = network.new_client().await.unwrap();
    #[cfg(feature = "dfu")]
    {
        let dfu = network.new_client().await.unwrap();
        spawner.spawn(updater_task(dfu, board.flash)).unwrap();
    }

    spawner.spawn(network_task(network)).unwrap();

    let device = DEVICE.put(TemperatureDevice::new());
    let config = TemperatureBoardConfig {
        send_trigger: TimeTrigger(Duration::from_secs(60)),
        sensor_ready: board.hts221_ready,
        sensor: Hts221::new(board.i2c2),
        network: client,
    };
    device.mount(spawner, TlsRand, config).await;

    defmt::info!("Application running");
}

#[embassy::task]
async fn network_task(adapter: &'static SharedEsWifi) {
    adapter.run().await;
}

#[cfg(feature = "dfu")]
#[embassy::task]
async fn updater_task(network: EsWifiClient, flash: Flash<'static>) {
    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);
    let updater = embassy_boot_stm32::FirmwareUpdater::default();
    drogue_update::run_updater::<_, _, _, 2048, 4096>(
        version.as_bytes(),
        updater,
        network,
        TlsRand,
        flash,
    )
    .await;
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
