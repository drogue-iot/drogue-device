#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]

use {
    drogue_device::{firmware::FirmwareManager, lora::*, ota::lorawan::*, *},
    embassy_boot_stm32::FirmwareUpdater,
    embassy_embedded_hal::adapter::BlockingAsync,
    embassy_executor::Spawner,
    embassy_stm32::flash::Flash,
    embassy_time::{Delay, Duration, Timer},
    embedded_storage::nor_flash::{NorFlash, ReadNorFlash},
    nucleo_wl55jc::*,
};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut board = NucleoWl55::default();

    let mut region: region::Configuration = region::EU868::default().into();

    // NOTE: This is specific for TTN, as they have a special RX1 delay
    region.set_receive_delay1(5000);

    let mut device = NucleoWl55::lorawan(region, board.radio, board.rng);

    // Depending on network, this might be part of JOIN
    device.set_datarate(region::DR::_0); // SF12

    defmt::info!("Joining LoRaWAN network");

    let join_mode = JoinMode::OTAA {
        deveui: EUI::from(DEV_EUI.trim_end()).0,
        appeui: EUI::from(APP_EUI.trim_end()).0,
        appkey: AppKey::from(APP_KEY.trim_end()).0,
    };
    board.blue_led.set_high();
    device.join(&join_mode).await.ok().unwrap();
    board.blue_led.set_low();
    defmt::info!("LoRaWAN network joined");

    let service = LorawanService::new(device);

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);

    let mut device: FirmwareManager<FirmwareConfig<Flash<'static>>, 4, 32> = FirmwareManager::new(
        FirmwareConfig::new(board.flash),
        FirmwareUpdater::default(),
        version.as_bytes(),
    );

    /// Matches fair usage policy of TTN
    const INTERVAL_MS: u32 = 1_124_000;

    let mut updater = embedded_update::FirmwareUpdater::new(
        service,
        embedded_update::UpdaterConfig {
            timeout_ms: 30_000,
            backoff_ms: INTERVAL_MS,
        },
    );

    loop {
        defmt::info!("Starting updater task");
        board.green_led.set_high();
        match updater.run(&mut device, &mut Delay).await {
            Ok(s) => match s {
                embedded_update::DeviceStatus::Updated => {
                    defmt::debug!("Resetting device");
                    cortex_m::peripheral::SCB::sys_reset();
                }
                embedded_update::DeviceStatus::Synced(_) => {}
            },
            Err(e) => {
                defmt::warn!("Error running updater: {:?}", e);
            }
        }
        board.green_led.set_low();
        Timer::after(Duration::from_millis(INTERVAL_MS as u64)).await;
    }
}

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

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

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.flash
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.flash
    }
}
