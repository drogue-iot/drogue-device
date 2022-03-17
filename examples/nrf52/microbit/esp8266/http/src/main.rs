#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

mod rng;
use rng::*;

use drogue_device::{actors::wifi::esp8266::*, drogue, traits::wifi::*, DeviceContext, Package};
use drogue_device::{
    actors::wifi::*,
    bsp::{boards::nrf52::microbit::*, Board},
    domain::temperature::Celsius,
    *,
};
use drogue_temperature::*;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, UARTE0},
    uarte,
    uarte::{Uarte, UarteRx, UarteTx},
    Peripherals,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type TX = UarteTx<'static, UARTE0>;
type RX = UarteRx<'static, UARTE0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

bind_bsp!(Microbit, BSP);

pub struct WifiDriver(Esp8266Wifi<TX, RX, ENABLE, RESET>);

impl Package for WifiDriver {
    type Configuration = <Esp8266Wifi<TX, RX, ENABLE, RESET> as Package>::Configuration;
    type Primary = <Esp8266Wifi<TX, RX, ENABLE, RESET> as Package>::Primary;

    fn mount<S: ActorSpawner>(
        &'static self,
        config: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        let wifi = self.0.mount(config, spawner);
        wifi.notify(AdapterRequest::Join(Join::Wpa {
            ssid: WIFI_SSID.trim_end(),
            password: WIFI_PSK.trim_end(),
        }))
        .unwrap();
        wifi
    }
}

impl TemperatureBoard for BSP {
    type NetworkPackage = WifiDriver;
    type Network = <WifiDriver as Package>::Primary;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = TemperatureMonitor;
    type SendTrigger = ButtonA;
    type Rng = Rng;
}

static DEVICE: DeviceContext<TemperatureDevice<BSP>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    let irq = interrupt::take!(UARTE0_UART0);
    let uart = Uarte::new_with_rtscts(
        board.uarte0,
        irq,
        board.p0_13,
        board.p0_01,
        board.p0_03,
        board.p0_04,
        config,
    );

    let (tx, rx) = uart.split();

    let enable_pin = Output::new(board.p0_09, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(board.p0_10, Level::Low, OutputDrive::Standard);

    let config = TemperatureBoardConfig {
        send_trigger: board.button_a,
        sensor: board.temp,
        sensor_ready: AlwaysReady,
        network_config: (),
    };

    #[cfg(feature = "tls")]
    defmt::info!("Application configured to use TLS");

    #[cfg(not(feature = "tls"))]
    defmt::info!("Application configured to NOT use TLS");

    DEVICE
        .configure(TemperatureDevice::new(WifiDriver(Esp8266Wifi::new(
            tx, rx, enable_pin, reset_pin,
        ))))
        .mount(
            spawner,
            Rng::new(nrf52833_pac::Peripherals::take().unwrap().RNG),
            config,
        )
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
