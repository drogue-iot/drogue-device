#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::drivers::sensors::hts221::Hts221;
use drogue_device::{
    bsp::{boards::stm32l4::iot01a::*, Board},
    domain::temperature::Celsius,
    traits::wifi::*,
    *,
};
use drogue_temperature::*;
use embassy::util::Forever;
use embassy_stm32::Peripherals;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

bind_bsp!(Iot01a, BSP);

impl TemperatureBoard for BSP {
    type Network = EsWifiClient;
    type TemperatureScale = Celsius;
    type SendTrigger = UserButton;
    type Sensor = Hts221<I2c2>;
    type SensorReadyIndicator = Hts221Ready;
    type Rng = Rng;
}

static DEVICE: DeviceContext<TemperatureDevice<BSP>> = DeviceContext::new();

#[embassy::main(config = "Iot01a::config(true)")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Iot01a::new(p);
    let mut wifi = board.wifi;

    match wifi.start().await {
        Ok(()) => defmt::info!("Started..."),
        Err(err) => defmt::info!("Error... {}", err),
    }

    defmt::info!("Joining WiFi network...");
    wifi.join(Join::Wpa {
        ssid: WIFI_SSID.trim_end(),
        password: WIFI_PSK.trim_end(),
    })
    .await
    .expect("Error joining wifi");
    defmt::info!("WiFi network joined");

    static NETWORK: Forever<SharedEsWifi> = Forever::new();
    let network = NETWORK.put(SharedEsWifi::new(wifi));
    let client = network.new_client().await.unwrap();
    spawner.spawn(network_task(network)).unwrap();

    let device = DEVICE.configure(TemperatureDevice::new());
    let config = TemperatureBoardConfig {
        send_trigger: board.user_button,
        sensor_ready: board.hts221_ready,
        sensor: Hts221::new(board.i2c2),
        network: client,
    };
    device.mount(spawner, board.rng, config).await;

    defmt::info!("Application initialized. Press 'User' button to send data");
}

#[embassy::task]
async fn network_task(adapter: &'static SharedEsWifi) {
    adapter.run().await;
}
