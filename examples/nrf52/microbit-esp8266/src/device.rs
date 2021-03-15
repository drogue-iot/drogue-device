use crate::app::*;
use drogue_device::{
    driver::{
        button::*,
        memory::{Memory, Query},
        timer::*,
        uart::serial::*,
        wifi::esp8266::Esp8266Wifi,
    },
    platform::cortex_m::nrf::{
        gpiote::*,
        timer::Timer as HalTimer,
        uarte::{UarteRx, UarteTx},
    },
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};

use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart = Serial<UarteTx<UARTE0>, UarteRx<UARTE0>, <AppTimer as Package>::Primary>;
pub type AppButton = Button<MyDevice, Pin<Input<PullUp>>>;
pub type Wifi =
    Esp8266Wifi<<AppUart as Package>::Primary, Pin<Output<PushPull>>, Pin<Output<PushPull>>>;
pub type AppWifi = <Wifi as Package>::Primary;

pub struct MyDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<AppButton>,
    pub btn_send: ActorContext<AppButton>,
    pub memory: ActorContext<Memory>,
    pub uart: AppUart,
    pub timer: AppTimer,
    pub wifi: Wifi,
    pub app: ActorContext<App<AppWifi>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_connect.mount(config.event_bus, supervisor);
        self.btn_send.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let wifi = self.wifi.mount(uart, supervisor);
        self.app.mount(wifi, supervisor);
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.memory.address().notify(Query);
        self.btn_send.address().notify(event);
        self.btn_connect.address().notify(event);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static self, event: ButtonEvent) {
        self.app.address().notify(event);
    }
}
