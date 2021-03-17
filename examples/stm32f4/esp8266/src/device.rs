use crate::app::*;
use drogue_device::{
    driver::{
        button::*,
        memory::{Memory, Query},
        timer::*,
        uart::serial::*,
        wifi::esp8266::Esp8266Wifi,
    },
    prelude::*,
};

pub struct MyDevice {}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {}
}
