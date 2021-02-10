use drogue_device::{
    device::DeviceConfiguration,
    domain::time::duration::Milliseconds,
    driver::{
        gpiote::nrf::*,
        led::{LEDMatrix, MatrixCommand},
        timer::{Timer, TimerActor},
        uart::dma::Uart,
    },
    hal::timer::nrf::Timer as HalTimer,
    hal::uart::nrf::Uarte as DmaUart,
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::TIMER0;
use heapless::consts;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type LedMatrix = LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, HalTimer<TIMER0>>;
pub type AppTimer = TimerActor<HalTimer<TIMER0>>;
pub type AppUart = <Uart<DmaUart<hal::pac::UARTE0>, HalTimer<TIMER0>> as Package>::Primary;

pub struct MyDevice {
    pub led: ActorContext<LedMatrix>,
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_fwd: ActorContext<Button>,
    pub btn_back: ActorContext<Button>,
    pub timer: Timer<HalTimer<TIMER0>>,
    pub uart: Uart<DmaUart<hal::pac::UARTE0>, HalTimer<TIMER0>>,
    pub app: ActorContext<App>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_fwd.mount(config.event_bus, supervisor);
        self.btn_back.mount(config.event_bus, supervisor);

        let timer = self.timer.mount((), supervisor);
        let display = self.led.mount(timer, supervisor);
        let uart = self.uart.mount(timer, supervisor);

        let app = self.app.mount(
            AppConfig {
                uart,
                display,
                timer,
            },
            supervisor,
        );

        app.notify(SayHello);
    }
}

impl EventHandler<GpioteEvent> for MyDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.btn_fwd.address().notify(event);
        self.btn_back.address().notify(event);
    }
}

impl EventHandler<PinEvent> for MyDevice {
    fn on_event(&'static self, event: PinEvent) {
        match event {
            PinEvent(Channel::Channel0, _) => {
                self.app.address().notify(StartService);
                self.led.address().notify(MatrixCommand::On(0, 0));
            }
            PinEvent(Channel::Channel1, _) => {
                self.led.address().notify(MatrixCommand::Off(0, 0));
            }
            _ => {}
        }
    }
}

pub struct AppConfig {
    uart: Address<AppUart>,
    display: Address<LedMatrix>,
    timer: Address<AppTimer>,
}

pub struct App {
    uart: Option<Address<AppUart>>,
    display: Option<Address<LedMatrix>>,
    timer: Option<Address<AppTimer>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            uart: None,
            display: None,
            timer: None,
        }
    }
}
impl Actor for App {
    type Configuration = AppConfig;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.uart);
        self.display.replace(config.display);
        self.timer.replace(config.timer);
        log::info!("Bound configuration");
    }
}

#[derive(Clone, Debug)]
pub struct SayHello;

#[derive(Clone, Debug)]
pub struct StartService;

impl NotifyHandler<SayHello> for App {
    fn on_notify(self, _: SayHello) -> Completion<Self> {
        Completion::defer(async move {
            let led = self.display.as_ref().unwrap();
            let timer = self.timer.as_ref().unwrap();

            for c in r"Hello, World!".chars() {
                led.notify(MatrixCommand::ApplyAscii(c));
                timer.delay(Milliseconds(200)).await;
            }

            led.notify(MatrixCommand::Clear);
            self
        })
    }
}

impl NotifyHandler<StartService> for App {
    fn on_notify(mut self, _: StartService) -> Completion<Self> {
        Completion::defer(async move {
            let led = self.display.as_ref().unwrap();
            let timer = self.timer.as_ref().unwrap();

            if let Some(uart) = &mut self.uart {
                let mut buf = [0; 128];

                let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
                buf[..motd.len()].clone_from_slice(motd);

                unsafe {
                    uart.write(&buf[..motd.len()])
                        .await
                        .map_err(|e| log::error!("Error writing MOTD: {:?}", e))
                        .ok();
                }

                loop {
                    // Assumes no more than 1 character typed per millisecond, just shorten the interval if need be!
                    let len = uart
                        .read_with_timeout(&mut buf[..], Milliseconds(100))
                        .await
                        .expect("Error reading from UART");

                    if len > 0 {
                        for b in &buf[..len] {
                            led.notify(MatrixCommand::ApplyAscii(*b as char));
                        }

                        uart.write(&buf[..len])
                            .await
                            .expect("Error writing to UART");
                    }
                }
            }
            self
        })
    }
}
