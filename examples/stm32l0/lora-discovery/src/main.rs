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
use embassy::executor::Spawner;
use embassy_stm32::{dbgmcu::Dbgmcu, Peripherals};
use lorawan_app::{LoraBoard, LoraDevice, LoraDeviceConfig};

bind_bsp!(LoraDiscovery, BSP);

static DEVICE: DeviceContext<LoraDevice<BSP>> = DeviceContext::new();

impl LoraBoard for BSP {
    type JoinLed = LedRed;
    type TxLed = LedGreen;
    type CommandLed = LedYellow;
    type SendTrigger = UserButton;
    type Driver = Device<'static, Radio, Rng>;
}

#[embassy::main(config = "LoraDiscovery::config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let board = LoraDiscovery::new(p);
    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN)
        .spreading_factor(SpreadingFactor::SF12);

    defmt::info!("Configuring with config {:?}", config);

    static mut RADIO_BUF: [u8; 256] = [0; 256];
    let config = LoraDeviceConfig {
        join_led: Some(board.led_red),
        tx_led: Some(board.led_green),
        command_led: Some(board.led_yellow),
        send_trigger: board.user_button,
        driver: unsafe { Device::new(&config, board.radio, board.rng, &mut RADIO_BUF).unwrap() },
    };

    DEVICE
        .configure(LoraDevice::new())
        .mount(spawner, config)
        .await;
}
