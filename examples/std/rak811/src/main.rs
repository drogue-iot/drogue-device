#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::cell::UnsafeCell;
use drogue_device::{
    drivers::lora::rak811::*, drogue, traits::lora::*, ActorContext, DeviceContext,
};
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

pub struct MyDevice {
    driver: UnsafeCell<Rak811Driver>,
    modem: ActorContext<Rak811ModemActor<'static, RX, RESET>>,
    app: ActorContext<App<Rak811Controller<'static, TX>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

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

    let device = DEVICE.configure(MyDevice {
        driver: UnsafeCell::new(Rak811Driver::new()),
        modem: ActorContext::new(),
        app: ActorContext::new(),
    });

    let (mut controller, modem) =
        unsafe { &mut *device.driver.get() }.initialize(tx, rx, reset_pin);
    device.modem.mount(spawner, Rak811ModemActor::new(modem));
    controller.configure(&config).await.unwrap();
    let app = device.app.mount(spawner, App::new(join_mode, controller));

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
