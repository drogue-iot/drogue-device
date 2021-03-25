use crate::app::*;
use drogue_device::{
    domain::time::duration::Milliseconds,
    driver::{
        button::{Button, ButtonEvent},
        timer::*,
        uart::serial::*,
        wifi::esp8266::Esp8266Wifi,
    },
    prelude::*,
};

use hal::{
    gpio::{
        gpioc::{PC10, PC12, PC13},
        Input, Output, PullUp, PushPull,
    },
    serial::{Rx, Tx},
    stm32::USART6,
};

use stm32f4xx_hal as hal;

// TODO:
pub type AppTimer = Timer<DummyTimer>;

type AppUart = Serial<Tx<USART6>, Rx<USART6>, <AppTimer as Package>::Primary>;
type Wifi =
    Esp8266Wifi<<AppUart as Package>::Primary, PC10<Output<PushPull>>, PC12<Output<PushPull>>>;
type AppWifi = <Wifi as Package>::Primary;
type ButtonInterrupt = Button<MyDevice, PC13<Input<PullUp>>>;

pub struct MyDevice {
    pub button: InterruptContext<ButtonInterrupt>,
    pub timer: AppTimer,
    pub uart: AppUart,
    pub wifi: Wifi,
    pub app: ActorContext<App<AppWifi>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.button.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let wifi = self.wifi.mount(uart, supervisor);
        self.app.mount(wifi, supervisor);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static self, event: ButtonEvent) {
        self.app.address().notify(event);
    }
}

pub struct DummyTimer;

impl drogue_device::hal::timer::Timer for DummyTimer {
    fn start(&mut self, _duration: Milliseconds) {}
    fn clear_update_interrupt_flag(&mut self) {}
}
