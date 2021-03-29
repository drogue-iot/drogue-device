use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use drogue_device::{
    api::{timer::Timer as TimerApi, uart::Uart},
    domain::time::duration::Milliseconds,
    driver::{
        //led::*,
        timer::*,
        uart::{serial_rx::*, serial_tx::*},
    },
    platform::cortex_m::nrf::{
        timer::Timer as HalTimer,
        uarte::{UarteRx, UarteTx},
    },
    prelude::*,
};
use hal::gpio::{Output, Pin as GpioPin, PushPull};
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
    U: Uart + 'static,
    D: TimerApi + 'static,
{
    uart: Address<U>,
    //    display: Address<LedMatrix>,
    timer: Address<D>,
}

pub struct App<U, D>
where
    U: Uart + 'static,
    D: TimerApi + 'static,
{
    uart: Option<Address<U>>,
    // display: Option<Address<LedMatrix>>,
    timer: Option<Address<D>>,
}

impl<U, D> App<U, D>
where
    U: Uart + 'static,
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
    U: Uart + 'static,
    D: TimerApi + 'static,
{
    type Request = SerialData;
    type Response = ();
    type ImmediateFuture = DefaultImmediate<Self>;
    type DeferredFuture = AppLogic<U, D>;
    type Configuration = AppConfig<U, D>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.uart);
        // self.display.replace(config.display);
        self.timer.replace(config.timer);
        log::info!("Application ready. Connect to the serial port to use the service.");
    }

    fn on_start(self) -> Completion<Self> {
        let uart = self.uart.as_ref().unwrap();
        let mut buf = [0; 128];
        let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
        buf[..motd.len()].clone_from_slice(motd);

        let fut = uart.write(&buf[..motd.len()]);
        /*
        .await
        .map_err(|e| log::error!("Error writing MOTD: {:?}", e))
        .ok();*/

        Completion::defer(AppLogic {
            app: Some(self),
            fut: fut,
        })
    }

    fn on_notify(self, event: Self::Request) -> Completion<Self> {
        let uart = self.uart.as_ref().unwrap();
        let mut buf = [0; 1];
        buf[0] = event.0;
        let fut = self.uart.as_ref().unwrap().write(&buf[..]);

        Completion::defer(AppLogic {
            app: Some(self),
            fut,
        })
    }
}

pub struct AppLogic<U, D>
where
    U: Uart + 'static,
    D: TimerApi + 'static,
{
    app: Option<App<U, D>>,
    fut: U::ImmediateFuture,
}

impl<U, D> Unpin for AppLogic<U, D>
where
    U: Uart + 'static,
    D: TimerApi + 'static,
{
}

impl<U, D> Future for AppLogic<U, D>
where
    U: Uart + 'static,
    D: TimerApi + 'static,
{
    type Output = (App<U, D>, ());

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fut.poll(cx) {
            Poll::Ready(val) => {
                log::info!("App logic polled future, got val: {:?}", val);
                (self.app.take().unwrap(), ())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
