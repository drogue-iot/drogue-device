#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{
    bsp::{boards::stm32wl::nucleo_wl55::*, Board},
    drivers::lora::LoraDevice as Device,
    firmware::{remote::LorawanService, FirmwareManager},
    traits::lora::{JoinMode, LoraConfig, LoraDriver, LoraMode, LoraRegion, SpreadingFactor},
};
use embassy::executor::Spawner;
use embassy::time::Delay;
use embassy::time::Duration;
use embassy::time::Timer;
use embassy_boot_stm32::FirmwareUpdater;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_stm32::flash::Flash;
use embassy_stm32::Peripherals;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

#[embassy::main(config = "NucleoWl55::config(true)")]
async fn main(_spawner: Spawner, p: Peripherals) {
    let mut board = NucleoWl55::new(p);

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    let mut driver = Device::new(&config, board.radio, board.rng).unwrap();

    defmt::info!("Joining LoRaWAN network");

    // TODO: Adjust the EUI and Keys according to your network credentials
    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };
    board.blue_led.on().ok();
    driver.join(join_mode).await.ok().unwrap();
    board.blue_led.off().ok();
    defmt::info!("LoRaWAN network joined");

    let service = LorawanService::new(driver);
    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    let mut device: FirmwareManager<FirmwareConfig<Flash<'static>>, 2048, 8> = FirmwareManager::new(
        FirmwareConfig::new(board.flash),
        FirmwareUpdater::default(),
        version.as_bytes(),
    );

    let mut updater = embedded_update::FirmwareUpdater::new(
        service,
        embedded_update::UpdaterConfig {
            timeout_ms: 30_000,
            backoff_ms: 10000,
        },
    );

    loop {
        defmt::info!("Starting updater task");
        board.green_led.on().ok();
        match updater.run(&mut device, &mut Delay).await {
            Ok(s) => {
                defmt::info!("Updater finished with status: {:?}", s);
            }
            Err(e) => {
                defmt::warn!("Error running updater: {:?}", e);
            }
        }
        board.green_led.off().ok();
        Timer::after(Duration::from_secs(300)).await;
    }
}

//const DEV_EUI: &str = drogue::config!("dev-eui");
//const APP_EUI: &str = drogue::config!("app-eui");
//const APP_KEY: &str = drogue::config!("app-key");
const APP_EUI: &str = "0000000000000000";
const DEV_EUI: &str = "72D71B613E086B02";
const APP_KEY: &str = "A935CC58C4D99A99F59594F38F3E37D5";

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
