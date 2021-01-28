use crate::gpiote::*;
use drogue_device::{
    driver::{
        led::{LEDMatrix, MatrixCommand},
        timer::Timer,
    },
    hal::timer::nrf::Timer as HalTimer,
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::TIMER0;
use heapless::consts;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type LedMatrix =
    LEDMatrix<MyDevice, Pin<Output<PushPull>>, consts::U5, consts::U5, HalTimer<TIMER0>>;
pub type TimerActor = Timer<MyDevice, HalTimer<TIMER0>>;

pub struct MyDevice {
    pub led: ActorContext<Self, LedMatrix>,
    pub gpiote: InterruptContext<Self, Gpiote<Self>>,
    pub btn_fwd: ActorContext<Self, Button>,
    pub btn_back: ActorContext<Self, Button>,
    pub timer: InterruptContext<Self, TimerActor>,
}

impl Device for MyDevice {
    fn mount(&'static mut self, bus: &EventBus<Self>, supervisor: &mut Supervisor) {
        let _gpiote_addr = self.gpiote.mount(bus, supervisor);
        let _fwd_addr = self.btn_fwd.mount(bus, supervisor);
        let _back_addr = self.btn_back.mount(bus, supervisor);
        let matrix_addr = self.led.mount(bus, supervisor);
        let timer_addr = self.timer.mount(bus, supervisor);

        matrix_addr.bind(&timer_addr);
    }
}

impl EventConsumer<GpioteEvent> for MyDevice {
    fn on_event(&'static mut self, event: GpioteEvent) {
        self.btn_fwd.address().notify(event);
        self.btn_back.address().notify(event);
    }
}

impl EventConsumer<PinEvent> for MyDevice {
    fn on_event(&'static mut self, event: PinEvent) {
        log::info!("Got pin event {:?}", event);
        match event {
            PinEvent(Channel::Channel0, _) => {
                log::info!("Notifying led");
                self.led.address().notify(MatrixCommand::On(0, 0));
            }
            PinEvent(Channel::Channel1, _) => {
                self.led.address().notify(MatrixCommand::Off(0, 0));
            }
            _ => {}
        }
    }
}

/*
impl EventConsumer<TimerEvent> for MyDevice {
    fn on_event(&'static mut self, event: TimerEvent) {
        self.led.address().notify(event);
    }
}
*/
/*
impl Actor for MyDevice {}

impl NotificationHandler<GpioteEvent> for MyDevice {
    fn on_notification(&'static mut self, event: GpioteEvent) -> Completion {
        Completion::immediate()
    }
}

*/
