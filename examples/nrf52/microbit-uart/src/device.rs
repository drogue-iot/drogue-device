use drogue_device::{
    api::{timer::Timer as TimerApi, uart::*},
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
pub type AppRx = SerialRx<App<AppTx, <AppTimer as Package>::Primary>, UarteRx<UARTE0>>;

/*
pub type LedMatrix =
    LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, <AppTimer as Package>::Primary>;*/

pub struct MyDevice {
    //    pub led: ActorContext<LedMatrix>,
    pub timer: AppTimer,
    pub tx: ActorContext<AppTx>,
    pub rx: InterruptContext<AppRx>,
    pub app: ActorContext<App<AppTx, <AppTimer as Package>::Primary>>,
}

impl Device for MyDevice {
    fn mount(&'static self, _: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer = self.timer.mount((), supervisor);
        //        let display = self.led.mount(timer, supervisor);
        let uart = self.tx.mount((), supervisor);
        let app = self.app.mount(
            AppConfig {
                uart,
                //        display,
                timer,
            },
            supervisor,
        );

        self.rx.mount(app, supervisor);
    }
}

pub struct AppConfig<U, D>
where
    U: UartWriter + 'static,
    D: TimerApi + 'static,
{
    uart: Address<U>,
    //    display: Address<LedMatrix>,
    timer: Address<D>,
}

pub struct App<U, D>
where
    U: UartWriter + 'static,
    D: TimerApi + 'static,
{
    uart: Option<Address<U>>,
    // display: Option<Address<LedMatrix>>,
    timer: Option<Address<D>>,
}

impl<U, D> App<U, D>
where
    U: UartWriter + 'static,
    D: TimerApi + 'static,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            //      display: None,
            timer: None,
        }
    }
}
impl<U, D> Actor for App<U, D>
where
    U: UartWriter + 'static,
    D: TimerApi + 'static,
{
    type Request = ();
    type Response = ();
    type Configuration = AppConfig<U, D>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.uart);
        // self.display.replace(config.display);
        self.timer.replace(config.timer);
        log::info!("Application ready. Connect to the serial port to use the service.");
    }

    fn on_start(self) -> Completion<Self> {
        Completion::defer(async move {
            //  let led = self.display.as_ref().unwrap();
            let timer = self.timer.as_ref().unwrap();
            let uart = self.uart.as_ref().unwrap();

            /*
            for c in r"Hello, World!".chars() {
                led.notify(Apply(c));
                timer.delay(Milliseconds(200)).await;
            }

            led.notify(Clear);
            */

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

    fn on_request(self, _: Self::Request) -> Response<Self> {
        Response::immediate(self, ())
    }
}

impl<U, D> RequestHandler<SerialData> for App<U, D>
where
    U: UartWriter + 'static,
    D: Delayer + 'static,
{
    type Response = ();
    fn on_request(self, event: SerialData) -> Response<Self, ()> {
        Response::defer(async move {
            /*
            self.display
                .as_ref()
                .unwrap()
                .notify(Apply(event.0 as char));*/
            let mut buf = [0; 1];
            buf[0] = event.0;
            self.uart
                .as_ref()
                .unwrap()
                .write(&buf[..])
                .await
                .expect("error writing data");
            (self, ())
        })
    }
}
