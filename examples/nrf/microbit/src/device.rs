use crate::gpiote::*;
use drogue_device::{
    driver::{
        led::{LEDMatrix, MatrixCommand},
        timer::Timer,
        uart::Uart,
    },
    hal::timer::nrf::Timer as HalTimer,
    hal::uart::nrf::Uarte as HalUart,
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::TIMER0;
use heapless::consts;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type LedMatrix = LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, HalTimer<TIMER0>>;
pub type TimerActor = Timer<HalTimer<TIMER0>>;

pub struct MyDevice {
    pub led: ActorContext<LedMatrix>,
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_fwd: ActorContext<Button>,
    pub btn_back: ActorContext<Button>,
    pub timer: InterruptContext<TimerActor>,
    pub uart: InterruptContext<Uart<HalUart<hal::pac::UARTE0>>>,
}

impl Device for MyDevice {
    fn mount(&'static mut self, bus: &Address<EventBus<Self>>, supervisor: &mut Supervisor) {
        self.gpiote.mount(supervisor).bind(bus);
        self.btn_fwd.mount(supervisor).bind(bus);
        self.btn_back.mount(supervisor).bind(bus);

        self.led
            .mount(supervisor)
            .bind(&self.timer.mount(supervisor));
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static mut self, event: GpioteEvent) {
        self.btn_fwd.address().notify(event);
        self.btn_back.address().notify(event);
    }
}

impl EventHandler<PinEvent> for MyDevice {
    fn on_event(&'static mut self, event: PinEvent) {
        match event {
            PinEvent(Channel::Channel0, _) => {
                self.led.address().notify(MatrixCommand::On(0, 0));
            }
            PinEvent(Channel::Channel1, _) => {
                self.led.address().notify(MatrixCommand::Off(0, 0));
            }
            _ => {}
        }
    }
}
