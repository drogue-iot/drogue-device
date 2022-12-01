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
    disco_l072z_lrwan1::*,
    drogue_device::{lora::*, *},
    embassy_executor::Spawner,
    embassy_futures::select::select,
    embassy_time::{Duration, Timer},
    heapless::String,
};

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = LoraDiscovery::default();

    let radio = Radio::new(
        board.spi1,
        board.radio_cs,
        board.radio_reset,
        board.radio_ready,
        DummySwitch,
    )
    .await
    .unwrap();

    let mut join_led = board.led_red;
    let mut tx_led = board.led_green;
    let mut command_led = board.led_yellow;
    let mut button = board.user_button;

    let mut region: region::Configuration = region::EU868::default().into();

    // NOTE: This is specific for TTN, as they have a special RX1 delay
    region.set_receive_delay1(5000);

    let mut device = LoraDiscovery::lorawan(region, radio, board.rng);

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
        select(Timer::after(Duration::from_secs(60)), button.wait_for_low()).await;
        counter += 1;

        defmt::info!("Sending message...");
        tx_led.set_high();

        let mut tx = String::<32>::new();
        write!(&mut tx, "ping:{}", counter).ok();
        defmt::info!("Message: {}", &tx.as_str());
        let tx = tx.into_bytes();

        let mut rx = [0; 64];
        let result = device.send_recv(&tx, &mut rx, 1, true).await;

        match result {
            Ok(rx_len) => {
                defmt::info!("Message sent!");
                if rx_len > 0 {
                    let response = &rx[0..rx_len];
                    match core::str::from_utf8(response) {
                        Ok(str) => {
                            defmt::info!("Received {} bytes from uplink:\n{}", rx_len, str)
                        }
                        Err(_) => defmt::info!(
                            "Received {} bytes from uplink: {:x}",
                            rx_len,
                            &rx[0..rx_len]
                        ),
                    }
                    match response {
                        b"led:on" => {
                            command_led.set_high();
                        }
                        b"led:off" => {
                            command_led.set_low();
                        }
                        _ => {}
                    }
                }
            }
            Err(_e) => {
                defmt::error!("Error sending message");
            }
        }

        tx_led.set_low();
    }
}
