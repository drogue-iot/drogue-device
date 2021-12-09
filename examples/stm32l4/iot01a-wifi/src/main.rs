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
    actors::wifi::*,
    bsp::{boards::stm32l4::iot01a::*, Board},
    domain::temperature::Celsius,
    traits::wifi::*,
    *,
};
use drogue_temperature::*;
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::Peripherals;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

bind_bsp!(Iot01a, BSP);

impl TemperatureBoard for BSP {
    type NetworkPackage = ActorContext<'static, AdapterActor<EsWifi>>;
    type Network = AdapterActor<EsWifi>;
    type TemperatureScale = Celsius;
    type SendTrigger = UserButton;
    type Sensor = Hts221<I2c2>;
    type SensorReadyIndicator = Hts221Ready;
    #[cfg(feature = "tls")]
    type Rng = Rng;
}

static DEVICE: DeviceContext<TemperatureDevice<BSP>> = DeviceContext::new();

#[embassy::main(config = "Iot01a::config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

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

    DEVICE.configure(TemperatureDevice::new(TemperatureBoardConfig {
        network: ActorContext::new(AdapterActor::new()),
        send_trigger: board.user_button,
        sensor_ready: board.hts221_ready,
        sensor: Hts221::new(board.i2c2),
    }));

    #[cfg(feature = "tls")]
    {
        let rng = board.rng;
        DEVICE
            .mount(|device| device.mount(spawner, wifi, rng))
            .await;
    }

    #[cfg(not(feature = "tls"))]
    DEVICE.mount(|device| device.mount(spawner, wifi)).await;

    defmt::info!("Application initialized. Press 'User' button to send data");
}
