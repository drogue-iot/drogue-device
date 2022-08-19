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
    traits::lora::{LoraConfig, LoraMode, LoraRegion, SpreadingFactor},
    *,
};
use drogue_lorawan_app::{LoraBoard, LoraDevice, LoraDeviceConfig};
use embassy_executor::Spawner;
use embassy_util::Forever;
use embassy_stm32::Peripherals;

bind_bsp!(NucleoWl55, BSP);

static DEVICE: Forever<LoraDevice<BSP>> = Forever::new();

impl LoraBoard for BSP {
    type JoinLed = LedBlue;
    type TxLed = LedGreen;
    type CommandLed = LedRed;
    type SendTrigger = UserButtonB1;
    type Driver = Device<Radio, Rng>;
}

#[embassy_executor::main(config = "NucleoWl55::config(true)")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = NucleoWl55::new(p);

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    let lora = Device::new(&config, board.radio, board.rng).unwrap();

    let config = LoraDeviceConfig {
        join_led: Some(board.blue_led),
        tx_led: Some(board.green_led),
        command_led: Some(board.red_led),
        send_trigger: board.user_button_b1,
        driver: lora,
    };
    DEVICE.put(LoraDevice::new()).mount(spawner, config).await;
}
