#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

mod serial;

use async_io::Async;
use core::cell::UnsafeCell;
use drogue_device::{drivers::wifi::esp8266::*, io::FromStdIo, traits::ip::*, *};
use embedded_hal::digital::v2::OutputPin;
use futures::io::BufReader;
use nix::sys::termios;
use serial::*;
use wifi_app::*;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const HOST: IpAddress = IpAddress::new_v4(192, 168, 1, 2);
const PORT: u16 = 12345;

type UART = FromStdIo<BufReader<Async<SerialPort>>>;
type ENABLE = DummyPin;
type RESET = DummyPin;

#[derive(Device)]
pub struct MyDevice {
    driver: UnsafeCell<Esp8266Driver>,
    modem: ActorContext<'static, Esp8266ModemActor<'static, UART, ENABLE, RESET>>,
    app: ActorContext<'static, App<Esp8266Controller<'static>>>,
}

#[drogue::main]
async fn main(context: DeviceContext<MyDevice>) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = BufReader::new(port);
    let port = FromStdIo::new(port);

    context.configure(MyDevice {
        driver: UnsafeCell::new(Esp8266Driver::new()),
        modem: ActorContext::new(Esp8266ModemActor::new()),
        app: ActorContext::new(App::new(
            WIFI_SSID.trim_end(),
            WIFI_PSK.trim_end(),
            HOST,
            PORT,
        )),
    });

    let app = context.mount(|device| {
        let (controller, modem) =
            unsafe { &mut *device.driver.get() }.initialize(port, DummyPin {}, DummyPin {});
        device.modem.mount(modem);
        device.app.mount(controller)
    });

    loop {
        app.request(Command::Send).unwrap().await;
        time::Timer::after(time::Duration::from_secs(10)).await;
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
