use drogue_device::{
    api::{delayer::*, lora::*, scheduler::*, uart::*},
    driver::{
        memory::{Memory, Query},
        timer::*,
        uart::dma::DmaUart,
        uart::*,
        wifi::esp8266::Esp8266Wifi,
    },
    platform::cortex_m::nrf::{gpiote::*, timer::Timer as HalTimer, uarte::Uarte as HalUart},
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;

use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart =
    DmaUart<HalUart<UARTE0>, <AppTimer as Package>::Primary, consts::U64, consts::U64>;
pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type AppWifi = Esp8266Wifi<
    <AppUart as Package>::Primary,
    <AppTimer as Package>::Primary,
    Pin<Output<PushPull>>,
    Pin<Output<PushPull>>,
>;

pub struct MyDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<Button>,
    pub btn_send: ActorContext<Button>,
    pub memory: ActorContext<Memory>,
    pub uart: AppUart,
    pub timer: AppTimer,
    pub wifi: AppWifi,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_connect.mount(config.event_bus, supervisor);
        self.btn_send.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        self.wifi.mount((uart, timer), supervisor);
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.btn_send.address().notify(event);
        self.btn_connect.address().notify(event);
    }
}

impl EventHandler<PinEvent> for MyDevice {
    fn on_event(&'static self, event: PinEvent) {
        match event {
            PinEvent(Channel::Channel0, PinState::Low) => {
                self.memory.address().notify(Query);
            }
            PinEvent(Channel::Channel1, PinState::Low) => {
                self.memory.address().notify(Query);
            }
            _ => {}
        }
    }
}
