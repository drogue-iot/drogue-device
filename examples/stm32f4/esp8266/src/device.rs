use crate::app::*;
use drogue_device::{
    domain::time::duration::Milliseconds,
    driver::{timer::*, uart::serial::*, wifi::esp8266::Esp8266Wifi},
    prelude::*,
};

use nucleo_f401re::{
    hal::{
        gpio::{
            gpioc::{PC10, PC12},
            Output, PushPull,
        },
        serial::{Rx, Tx},
    },
    pac::USART6,
};

// TODO:
pub type AppTimer = Timer<DummyTimer>;

type AppUart = Serial<Tx<USART6>, Rx<USART6>, <AppTimer as Package>::Primary>;
type Wifi =
    Esp8266Wifi<<AppUart as Package>::Primary, PC10<Output<PushPull>>, PC12<Output<PushPull>>>;
type AppWifi = <Wifi as Package>::Primary;

pub struct MyDevice {
    pub timer: AppTimer,
    pub uart: AppUart,
    pub wifi: Wifi,
    pub app: ActorContext<App<AppWifi>>,
}

impl Device for MyDevice {
    fn mount(&'static self, _config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let wifi = self.wifi.mount(uart, supervisor);
        self.app.mount(wifi, supervisor);
    }
}

pub struct DummyTimer;

impl drogue_device::hal::timer::Timer for DummyTimer {
    fn start(&mut self, _duration: Milliseconds) {}
    fn clear_update_interrupt_flag(&mut self) {}
}
