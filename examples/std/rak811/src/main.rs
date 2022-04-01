#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{drivers::lora::rak811::*, drogue, traits::lora::*, ActorContext};
use embassy::time::{Duration, Timer};
use embedded_hal::digital::v2::OutputPin;

mod app;
mod serial;

use app::*;
use nix::sys::termios;
use serial::*;

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

type TX = SerialWriter;
type RX = SerialReader;
type RESET = DummyPin;

static APP: ActorContext<App<Rak811Controller<'static, TX>>> = ActorContext::new();
static mut DRIVER: Rak811Driver = Rak811Driver::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let (tx, rx) = port.split();

    let reset_pin = DummyPin {};

    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    let (mut controller, m) = unsafe { &mut DRIVER }.initialize(tx, rx, reset_pin);
    spawner.spawn(modem(m)).unwrap();

    controller.configure(&config).await.unwrap();

    let app = APP.mount(spawner, App::new(join_mode, controller));

    loop {
        let _ = app.notify(AppCommand::Send);
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy::task]
async fn modem(mut m: Rak811Modem<'static, RX, RESET>) {
    m.run().await;
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
