use drogue_device::{
    api::{delayer::*, uart::*},
    domain::time::duration::Milliseconds,
    driver::{led::*, timer::*, uart::dma::*},
    platform::cortex_m::nrf::{gpiote::*, timer::Timer as HalTimer, uarte::Uarte},
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<MyDevice, Pin<Input<PullUp>>>;
pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart = DmaUart<Uarte<UARTE0>, <AppTimer as Package>::Primary, consts::U64, consts::U64>;
pub type LedMatrix =
    LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, <AppTimer as Package>::Primary>;

pub struct MyDevice {
    pub led: ActorContext<LedMatrix>,
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_fwd: ActorContext<Button>,
    pub btn_back: ActorContext<Button>,
    pub timer: AppTimer,
    pub uart: AppUart,
    pub app: ActorContext<App<<AppUart as Package>::Primary, <AppTimer as Package>::Primary>>,
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
                self.led.address().notify(On(0, 0));
            }
            PinEvent(Channel::Channel1, _) => {
                self.led.address().notify(Off(0, 0));
            }
            _ => {}
        }
    }
}

pub struct AppConfig<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer + 'static,
{
    uart: Address<U>,
    display: Address<LedMatrix>,
    timer: Address<D>,
}

pub struct App<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer + 'static,
{
    uart: Option<Address<U>>,
    display: Option<Address<LedMatrix>>,
    timer: Option<Address<D>>,
}

impl<U, D> App<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            display: None,
            timer: None,
        }
    }
}
impl<U, D> Actor for App<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer,
{
    type Configuration = AppConfig<U, D>;
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

impl<U, D> NotifyHandler<SayHello> for App<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer,
{
    fn on_notify(self, _: SayHello) -> Completion<Self> {
        Completion::defer(async move {
            let led = self.display.as_ref().unwrap();
            let timer = self.timer.as_ref().unwrap();

            for c in r"Hello, World!".chars() {
                led.notify(Apply(c));
                timer.delay(Milliseconds(200)).await;
            }

            led.notify(Clear);
            self
        })
    }
}

impl<U, D> NotifyHandler<StartService> for App<U, D>
where
    U: UartWriter + UartReader + 'static,
    D: Delayer,
{
    fn on_notify(mut self, _: StartService) -> Completion<Self> {
        Completion::defer(async move {
            let led = self.display.as_ref().unwrap();

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

                let mut rx_buf = [0; 128];
                loop {
                    // Shorten the interval or reduce size of rx buf if more responsiveness is needed.
                    let len = uart
                        .read_with_timeout(&mut rx_buf[..], Milliseconds(100))
                        .await
                        .expect("Error reading from UART");

                    if len > 0 {
                        for b in &rx_buf[..len] {
                            led.notify(Apply(*b as char));
                        }

                        uart.write(&rx_buf[..len])
                            .await
                            .expect("Error writing to UART");
                    }
                }
            }
            self
        })
    }
}
