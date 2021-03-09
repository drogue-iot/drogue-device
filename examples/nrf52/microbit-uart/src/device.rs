use drogue_device::{
    api::{delayer::*, uart::*},
    domain::time::duration::Milliseconds,
    driver::{
        led::*,
        timer::*,
        uart::{serial_rx::*, serial_tx::*},
    },
    platform::cortex_m::nrf::{
        timer::Timer as HalTimer,
        uarte::{UarteRx, UarteTx},
    },
    prelude::*,
};
use hal::gpio::{Output, Pin, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;
use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppTx = SerialTx<UarteTx<UARTE0>>;
pub type AppRx = SerialRx<MyDevice, UarteRx<UARTE0>>;

pub type LedMatrix =
    LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, <AppTimer as Package>::Primary>;

pub struct MyDevice {
    pub led: ActorContext<LedMatrix>,
    pub timer: AppTimer,
    pub tx: ActorContext<AppTx>,
    pub rx: InterruptContext<AppRx>,
    pub app: ActorContext<App<AppTx, <AppTimer as Package>::Primary>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer = self.timer.mount((), supervisor);
        let display = self.led.mount(timer, supervisor);
        let uart = self.tx.mount((), supervisor);
        self.rx.mount(config.event_bus, supervisor);

        self.app.mount(
            AppConfig {
                uart,
                display,
                timer,
            },
            supervisor,
        );
    }
}

impl EventHandler<SerialData> for MyDevice {
    fn on_event(&'static self, event: SerialData) {
        self.led.address().notify(Apply(event.0 as char));
        self.tx.address().notify(event);
    }
}

pub struct AppConfig<U, D>
where
    U: UartWriter + 'static,
    D: Delayer + 'static,
{
    uart: Address<U>,
    display: Address<LedMatrix>,
    timer: Address<D>,
}

pub struct App<U, D>
where
    U: UartWriter + 'static,
    D: Delayer + 'static,
{
    uart: Option<Address<U>>,
    display: Option<Address<LedMatrix>>,
    timer: Option<Address<D>>,
}

impl<U, D> App<U, D>
where
    U: UartWriter + 'static,
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
    U: UartWriter + 'static,
    D: Delayer,
{
    type Configuration = AppConfig<U, D>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.uart);
        self.display.replace(config.display);
        self.timer.replace(config.timer);
        log::info!("Application ready. Connect to the serial port to use the service.");
    }

    fn on_start(self) -> Completion<Self> {
        Completion::defer(async move {
            let led = self.display.as_ref().unwrap();
            let timer = self.timer.as_ref().unwrap();
            let uart = self.uart.as_ref().unwrap();

            for c in r"Hello, World!".chars() {
                led.notify(Apply(c));
                timer.delay(Milliseconds(200)).await;
            }

            led.notify(Clear);

            let mut buf = [0; 128];
            let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);

            uart.write(&buf[..motd.len()])
                .await
                .map_err(|e| log::error!("Error writing MOTD: {:?}", e))
                .ok();

            self
        })
    }
}
