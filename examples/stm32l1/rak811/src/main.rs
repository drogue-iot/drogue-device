#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    bsp::{boards::stm32l1::rak811::*, Board},
    drivers::lora::LoraDevice as Device,
    traits::lora::{LoraConfig, LoraMode, LoraRegion, SpreadingFactor},
    *,
};
use drogue_lorawan_app::{LoraBoard, LoraDevice, LoraDeviceConfig, TimeTrigger};
use embassy_executor::executor::Spawner;
use embassy_executor::time::Duration;
use embassy_util::Forever;
use embassy_stm32::Peripherals;

bind_bsp!(Rak811, BSP);

static DEVICE: Forever<LoraDevice<BSP>> = Forever::new();

impl LoraBoard for BSP {
    type JoinLed = LedRed;
    type TxLed = LedRed;
    type CommandLed = LedRed;
    type SendTrigger = TimeTrigger;
    type Driver = Device<Radio, Rng>;
}

#[embassy_executor::main(config = "Rak811::config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = Rak811::new(p);

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    static mut RADIO_BUF: [u8; 256] = [0; 256];
    let radio = Radio::new(
        board.spi1,
        board.radio_cs,
        board.radio_reset,
        board.radio_ready,
        board.radio_switch,
    )
    .await
    .unwrap();
    let lora = Device::new(&config, radio, board.rng).unwrap();
    let config = LoraDeviceConfig {
        join_led: Some(board.led_red),
        tx_led: None,
        command_led: None,
        send_trigger: TimeTrigger(Duration::from_secs(60)),
        driver: lora,
    };
    DEVICE.put(LoraDevice::new()).mount(spawner, config).await;
}
