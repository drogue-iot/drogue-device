#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod serial;

use async_io::Async;
use drogue_device::{
    actors::wifi::{esp8266::*, AdapterRequest},
    domain::temperature::Celsius,
    traits::wifi::*,
    *,
};
use drogue_temperature::*;
use embassy::io::FromStdIo;
use embassy::time::Duration;
use embedded_hal::digital::v2::OutputPin;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type UART = FromStdIo<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

pub struct StdBoard;

impl TemperatureBoard for StdBoard {
    type NetworkPackage = WifiDriver;
    type Network = <WifiDriver as Package>::Primary;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = TimeTrigger;
    #[cfg(feature = "tls")]
    type Rng = rand::rngs::OsRng;
}

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

static DEVICE: DeviceContext<TemperatureDevice<StdBoard>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = BufReader::new(port);
    let port = FromStdIo::new(port);

    DEVICE.configure(TemperatureDevice::new(TemperatureBoardConfig {
        network: WifiDriver(Esp8266Wifi::new(port, DummyPin {}, DummyPin {})),
        send_trigger: TimeTrigger(Duration::from_secs(10)),
        sensor: FakeSensor(22.0),
        sensor_ready: AlwaysReady,
    }));

    #[cfg(feature = "tls")]
    DEVICE
        .mount(|device| device.mount(spawner, (), rand::rngs::OsRng))
        .await;

    #[cfg(not(feature = "tls"))]
    DEVICE.mount(|device| device.mount(spawner, ())).await;
}

pub struct DummyPin {}
impl OutputPin for DummyPin {
    type Error = ();
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
