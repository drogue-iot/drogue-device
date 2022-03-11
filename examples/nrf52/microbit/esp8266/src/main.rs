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
use embassy::util::Forever;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, TIMER0, UARTE0},
    uarte, Peripherals,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

bind_bsp!(Microbit, BSP);

pub struct WifiDriver(Esp8266Wifi<UART, ENABLE, RESET>);

impl Package for WifiDriver {
    type Configuration = <Esp8266Wifi<UART, ENABLE, RESET> as Package>::Configuration;
    type Primary = <Esp8266Wifi<UART, ENABLE, RESET> as Package>::Primary;

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

    static mut TX_BUFFER: [u8; 8192] = [0u8; 8192];
    static mut RX_BUFFER: [u8; 8192] = [0u8; 8192];
    static mut STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();

    let irq = interrupt::take!(UARTE0_UART0);
    let u = unsafe {
        let state = STATE.put(State::new());
        BufferedUarte::new_without_flow_control(
            state,
            board.uarte0,
            board.timer0,
            board.ppi_ch0,
            board.ppi_ch1,
            irq,
            board.p0_13,
            board.p0_01,
            config,
            &mut RX_BUFFER,
            &mut TX_BUFFER,
        )
    };

    let enable_pin = Output::new(board.p0_09, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(board.p0_10, Level::Low, OutputDrive::Standard);

    let config = TemperatureBoardConfig {
        send_trigger: board.button_a,
        sensor: board.temp,
        sensor_ready: AlwaysReady,
        network_config: (),
    };

    DEVICE
        .configure(TemperatureDevice::new(WifiDriver(Esp8266Wifi::new(
            u, enable_pin, reset_pin,
        ))))
        .mount(
            spawner,
            Rng::new(nrf52833_pac::Peripherals::take().unwrap().RNG),
            config,
        )
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
