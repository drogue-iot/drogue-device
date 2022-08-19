#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use async_io::Async;
use drogue_device::{drivers::lora::rak811::*, drogue, traits::lora::*};
use ector::ActorContext;
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::OutputPin;
use embedded_io::adapters::FromFutures;
use futures::io::BufReader;

mod app;
mod serial;

use app::*;
use nix::sys::termios;
use serial::*;

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

type SERIAL = FromFutures<BufReader<Async<SerialPort>>>;
type RESET = DummyPin;

static APP: ActorContext<App<Rak811Modem<SERIAL, RESET>>> = ActorContext::new();

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

    let reset_pin = DummyPin {};

    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    let mut modem = Rak811Modem::new(port, reset_pin);
    modem.initialize().await.unwrap();
    modem.configure(&config).await.unwrap();

    let app = APP.mount(spawner, App::new(join_mode, modem));

    loop {
        let _ = app.notify(AppCommand::Send);
        Timer::after(Duration::from_secs(60)).await;
    }
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
