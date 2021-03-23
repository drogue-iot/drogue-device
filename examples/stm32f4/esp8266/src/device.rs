use crate::app::*;
use drogue_device::{
    domain::time::duration::Milliseconds,
    driver::{
        button::*,
        memory::{Memory, Query},
        timer::*,
        uart::serial::*,
        wifi::esp8266::Esp8266Wifi,
    },
    prelude::*,
};

use nucleo_f401re::{
    hal,
    hal::{
        gpio::{
            gpioc::{PC10, PC12},
            Alternate, Floating, Input, OpenDrain, Output, PullDown, PullUp, PushPull,
        },
        prelude::*,
        serial::{Rx, Serial as NucleoSerial, Tx},
    },
    pac::USART6,
};

// TODO:
pub type AppTimer = Timer<DummyTimer>;

pub type AppUart = Serial<Tx<USART6>, Rx<USART6>, <AppTimer as Package>::Primary>;
pub type Wifi =
    Esp8266Wifi<<AppUart as Package>::Primary, PC10<Output<PushPull>>, PC12<Output<PushPull>>>;

pub struct MyDevice {
    pub uart: AppUart,
    pub wifi: Wifi,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {}
}

pub struct DummyTimer;

impl drogue_device::hal::timer::Timer for DummyTimer {
    fn start(&mut self, duration: Milliseconds) {}
    fn clear_update_interrupt_flag(&mut self) {}
}
