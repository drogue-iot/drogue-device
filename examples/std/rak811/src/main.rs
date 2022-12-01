#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {
    async_io::Async,
    drogue_device::*,
    embassy_time::{Duration, Timer},
    embedded_hal::digital::{ErrorType, OutputPin},
    embedded_io::adapters::FromFutures,
    rak811_at_driver::*,
};

mod serial;

use {nix::sys::termios, serial::*};

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
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
        dev_eui: EUI::from(DEV_EUI.trim_end()),
        app_eui: EUI::from(APP_EUI.trim_end()),
        app_key: AppKey::from(APP_KEY.trim_end()),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    let mut modem = Rak811Driver::new(port, reset_pin);
    modem.initialize().await.unwrap();
    modem.configure(&config).await.unwrap();

    log::info!("Joining LoRaWAN network");
    modem.join(join_mode).await.unwrap();
    log::info!("LoRaWAN network joined");

    loop {
        log::info!("Sending data..");
        let result = modem.send(QoS::Confirmed, 1, b"ping").await;
        log::info!("Data sent: {:?}", result);
        Timer::after(Duration::from_secs(60)).await;
    }
}

pub struct DummyPin {}
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
