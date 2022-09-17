#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    bsp::{boards::stm32l0::lora_discovery::*, Board},
    drivers::lora::LoraDevice as Device,
    traits::lora::{LoraConfig, LoraMode, LoraRegion, SpreadingFactor},
    *,
};
use drogue_lorawan_app::{LoraBoard, LoraDevice, LoraDeviceConfig};
use embassy_executor::Spawner;
use static_cell::StaticCell;

bind_bsp!(LoraDiscovery, BSP);

static DEVICE: StaticCell<LoraDevice<BSP>> = StaticCell::new();

impl LoraBoard for BSP {
    type JoinLed = LedRed;
    type TxLed = LedGreen;
    type CommandLed = LedYellow;
    type SendTrigger = UserButton;
    type Driver = Device<Radio, Rng>;
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = LoraDiscovery::new(embassy_stm32::init(LoraDiscovery::config()));
    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    let radio = Radio::new(
        board.spi1,
        board.radio_cs,
        board.radio_reset,
        board.radio_ready,
        DummySwitch,
    )
    .await
    .unwrap();

    let config = LoraDeviceConfig {
        join_led: Some(board.led_red),
        tx_led: Some(board.led_green),
        command_led: Some(board.led_yellow),
        send_trigger: board.user_button,
        driver: Device::new(&config, radio, board.rng).unwrap(),
    };

    DEVICE.init(LoraDevice::new()).mount(spawner, config).await;
}
