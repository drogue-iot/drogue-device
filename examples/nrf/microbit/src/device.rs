use crate::gpiote::*;
use drogue_device::{
    driver::{
        led::{LEDMatrix, MatrixCommand},
        timer::Timer,
        uart::{Error as UartError, Uart, UartPeripheral},
    },
    hal::timer::nrf::Timer as HalTimer,
    hal::uart::nrf::Uarte as HalUart,
    prelude::*,
    synchronization::Mutex,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::TIMER0;
use heapless::{consts, String};
use nrf52833_hal as hal;

pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type LedMatrix = LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, HalTimer<TIMER0>>;
pub type TimerActor = Timer<HalTimer<TIMER0>>;
pub type AppUart = Mutex<UartPeripheral<HalUart<hal::pac::UARTE0>>>;

pub struct MyDevice {
    pub led: ActorContext<LedMatrix>,
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_fwd: ActorContext<Button>,
    pub btn_back: ActorContext<Button>,
    pub timer: InterruptContext<TimerActor>,
    pub uart: Uart<HalUart<hal::pac::UARTE0>>,
    pub app: ActorContext<App>,
}

impl Device for MyDevice {
    fn mount(&'static mut self, bus: &Address<EventBus<Self>>, supervisor: &mut Supervisor) {
        self.gpiote.mount(supervisor).bind(bus);
        self.btn_fwd.mount(supervisor).bind(bus);
        self.btn_back.mount(supervisor).bind(bus);
        self.app
            .mount(supervisor)
            .bind(&self.uart.mount(bus, supervisor));

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
                self.app.address().notify(SayHello);
                self.led.address().notify(MatrixCommand::On(0, 0));
            }
            PinEvent(Channel::Channel1, _) => {
                self.led.address().notify(MatrixCommand::Off(0, 0));
            }
            _ => {}
        }
    }
}

pub struct App {
    uart: Option<Address<AppUart>>,
}

impl App {
    pub fn new() -> Self {
        Self { uart: None }
    }
}
impl Actor for App {}

#[derive(Debug)]
pub struct SayHello;
impl Bind<AppUart> for App {
    fn on_bind(&'static mut self, address: Address<AppUart>) {
        log::info!("Bound uart");
        self.uart.replace(address);
    }
}

impl NotifyHandler<SayHello> for App {
    fn on_notify(&'static mut self, message: SayHello) -> Completion {
        Completion::defer(async move {
            if let Some(uart) = &mut self.uart {
                let mut buf = [0; 128];
                let mut uart = uart.lock().await;

                let motd = "Welcome to UART Echo Service\r\n".as_bytes();
                buf[..motd.len()].clone_from_slice(motd);
                let result = uart.write(&buf[..motd.len()]).await;
                log::info!("MOTD RESULT: {:?}", result);

                let result = uart.read(&mut buf[..1]).await;
                log::info!("RX RESULT: {:?}", result);
                let result = uart.write(&buf[..1]).await;
                log::info!("TX RESULT: {:?}", result);
            }
        })
    }
}
