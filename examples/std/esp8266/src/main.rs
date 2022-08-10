#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod serial;

use async_io::Async;
use drogue_device::{domain::temperature::Celsius, drivers::wifi::esp8266::*, *};
use drogue_temperature::*;
use embassy_executor::time::Duration;
use embassy_util::Forever;
use embedded_hal::digital::v2::OutputPin;
use embedded_io::adapters::FromFutures;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type SERIAL = FromFutures<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

pub struct StdBoard;

impl TemperatureBoard for StdBoard {
    type Network = &'static Esp8266Modem<'static, SERIAL, ENABLE, RESET, 1>;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = TimeTrigger;
    type Rng = rand::rngs::OsRng;
}

static DEVICE: Forever<TemperatureDevice<StdBoard>> = Forever::new();

#[embassy_executor::main]
async fn main(spawner: embassy_executor::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = futures::io::BufReader::new(port);
    let port = FromFutures::new(port);

    let network = Esp8266Modem::new(port, DummyPin, DummyPin);
    static NETWORK: Forever<Esp8266Modem<SERIAL, ENABLE, RESET, 1>> = Forever::new();
    let network = NETWORK.put(network);
    spawner
        .spawn(net_task(network, WIFI_SSID.trim_end(), WIFI_PSK.trim_end()))
        .unwrap();

    DEVICE
        .put(TemperatureDevice::new())
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
    modem: &'static Esp8266Modem<'static, SERIAL, ENABLE, RESET, 1>,
    ssid: &'static str,
    psk: &'static str,
) {
    loop {
        let _ = modem.run(ssid, psk).await;
    }
}

pub struct DummyPin;
impl OutputPin for DummyPin {
    type Error = ();
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
