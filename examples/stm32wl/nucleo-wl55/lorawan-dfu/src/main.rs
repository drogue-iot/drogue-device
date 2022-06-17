#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    bsp::{boards::stm32wl::nucleo_wl55::*, Board},
    drivers::lora::LoraDevice as Device,
    firmware::FirmwareManager,
    traits::lora::{JoinMode, LoraConfig, LoraDriver, LoraMode, LoraRegion, SpreadingFactor},
    *,
};
use embassy::executor::Spawner;
use embassy::time::Delay;
use embassy::time::Duration;
use embassy::time::Timer;
use embassy::util::Forever;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_stm32::Peripherals;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};

#[embassy::main(config = "NucleoWl55::config(true)")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = NucleoWl55::new(p);

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
    driver.join(join_mode).await.ok().unwrap();
    defmt::info!("LoRaWAN network joined");

    let mut service = LorawanService::new(driver);

    let mut device: FirmwareManager<FirmwareConfig<F>, 2048, 128> =
        FirmwareManager::new(FirmwareConfig::new(board.flash), updater, version);

    let mut updater = embedded_update::FirmwareUpdater::new(
        service,
        embedded_update::UpdaterConfig {
            timeout_ms: 120_000,
            backoff_ms: 100,
        },
    );

    loop {
        defmt::info!("Starting updater task");
        match updater.run(&mut device, &mut Delay).await {
            Ok(s) => {
                defmt::info!("Updater finished with status: {:?}", s);
            }
            Err(e) => {
                defmt::warn!("Error running updater: {:?}", e);
            }
        }
        Timer::after(Duration::from_secs(10)).await;
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
    const BLOCK_SIZE: usize = F::ERASE_SIZE;

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.flash
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.flash
    }
}
