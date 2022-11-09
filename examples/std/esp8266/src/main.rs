#![macro_use]
#![feature(type_alias_impl_trait)]

mod serial;

use async_io::Async;
use drogue_device::{domain::temperature::Celsius, *};
use drogue_temperature::*;
use embassy_time::Duration;
use embedded_hal::digital::{ErrorType, OutputPin};
use embedded_io::adapters::FromFutures;
use esp8266_at_driver::*;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;
use static_cell::StaticCell;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type SERIAL = FromFutures<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

pub struct StdBoard;

impl TemperatureBoard for StdBoard {
    type Network = &'static Esp8266Driver<'static, SERIAL, ENABLE, RESET, 1>;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = TimeTrigger;
    type Rng = rand::rngs::OsRng;
}

static DEVICE: StaticCell<TemperatureDevice<StdBoard>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = futures::io::BufReader::new(port);
    let port = FromFutures::new(port);

    let network = Esp8266Driver::new(port, DummyPin, DummyPin);
    static NETWORK: StaticCell<Esp8266Driver<SERIAL, ENABLE, RESET, 1>> = StaticCell::new();
    let network = NETWORK.init(network);
    spawner
        .spawn(net_task(network, WIFI_SSID.trim_end(), WIFI_PSK.trim_end()))
        .unwrap();

    DEVICE
        .init(TemperatureDevice::new())
        .mount(
            spawner,
            rand::rngs::OsRng,
            TemperatureBoardConfig {
                network,
                send_trigger: TimeTrigger(Duration::from_secs(10)),
                sensor: FakeSensor(22.0),
                sensor_ready: AlwaysReady,
            },
        )
        .await;
}

#[embassy_executor::task]
async fn net_task(
    modem: &'static Esp8266Driver<'static, SERIAL, ENABLE, RESET, 1>,
    ssid: &'static str,
    psk: &'static str,
) {
    loop {
        let _ = modem.run(ssid, psk).await;
    }
}

pub struct DummyPin;
impl ErrorType for DummyPin {
    type Error = ();
}

impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
