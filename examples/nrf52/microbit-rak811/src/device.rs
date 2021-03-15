use drogue_device::{
    driver::{
        button::*,
        lora::*,
        memory::{Memory, Query},
        timer::*,
        uart::dma::DmaUart,
    },
    platform::cortex_m::nrf::{gpiote::*, timer::Timer as HalTimer, uarte::Uarte as HalUart},
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;
use lora_common::*;

use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart =
    DmaUart<HalUart<UARTE0>, <AppTimer as Package>::Primary, consts::U64, consts::U64>;
pub type Rak811Lora = rak811::Rak811<<AppUart as Package>::Primary, Pin<Output<PushPull>>>;
pub type AppLora = <Rak811Lora as Package>::Primary;

pub struct MyDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub button: ActorContext<Button<Self, Pin<Input<PullUp>>>>,
    pub memory: ActorContext<Memory>,
    pub uart: AppUart,
    pub lora: Rak811Lora,
    pub timer: AppTimer,
    pub app: ActorContext<App<AppLora>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.button.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let lora = self.lora.mount(uart, supervisor);
        self.app.mount(lora, supervisor);
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.button.address().notify(event);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static self, event: ButtonEvent) {
        match event {
            ButtonEvent::Pressed => {
                self.memory.address().notify(Query);
            }
            _ => {}
        }
        self.app.address().notify(event)
    }
}
