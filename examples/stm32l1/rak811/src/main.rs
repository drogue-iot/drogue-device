#![no_std]
#![no_main]
#![macro_use]
#![allow(dead_code)]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {defmt_rtt as _, panic_probe as _};

use {
    core::fmt::Write as _,
    drogue_device::{lora::*, *},
    embassy_executor::Spawner,
    embassy_time::{Duration, Timer},
    heapless::String,
    rak811::*,
};

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = Rak811::default();

    let radio = Radio::new(
        board.spi1,
        board.radio_cs,
        board.radio_reset,
        board.radio_ready,
        board.radio_switch,
    )
    .await
    .unwrap();

    let mut join_led = board.led_red;
    let mut region: region::Configuration = region::EU868::default().into();

    // NOTE: This is specific for TTN, as they have a special RX1 delay
    region.set_receive_delay1(5000);

    let mut device = Rak811::lorawan(region, radio, board.rng);

    let join_mode = JoinMode::OTAA {
        deveui: EUI::from(DEV_EUI.trim_end()).0,
        appeui: EUI::from(APP_EUI.trim_end()).0,
        appkey: AppKey::from(APP_KEY.trim_end()).0,
    };

    join_led.set_high();
    defmt::info!("Joining LoRaWAN network");
    device.join(&join_mode).await.ok().unwrap();
    defmt::info!("LoRaWAN network joined");
    join_led.set_low();

    let mut counter = 0;
    loop {
        counter += 1;

        let mut tx = String::<32>::new();
        write!(&mut tx, "ping:{}", counter).ok();
        defmt::info!("Sending message: {}", &tx.as_str());
        let tx = tx.into_bytes();

        let result = device.send(&tx, 1, true).await;
        match result {
            Ok(_) => {
                defmt::info!("Message sent!");
            }
            Err(_e) => {
                defmt::error!("Error sending message");
            }
        }
        Timer::after(Duration::from_secs(60)).await;
    }
}
