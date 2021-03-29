use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use drogue_device::{
    api::uart::Uart,
    domain::time::duration::Milliseconds,
    driver::uart::{serial_rx::*, serial_tx::*},
    platform::cortex_m::nrf::uarte::{UarteRx, UarteTx},
    prelude::*,
};
use hal::gpio::{Output, Pin as GpioPin, PushPull};
use hal::pac::UARTE0;
use heapless::consts;
use nrf52833_hal as hal;

pub type AppTx = SerialTx<UarteTx<UARTE0>>;
pub type AppRx = SerialRx<App<AppTx>, UarteRx<UARTE0>>;

/*
pub type LedMatrix =
    LEDMatrix<Pin<Output<PushPull>>, consts::U5, consts::U5, <AppTimer as Package>::Primary>;*/

pub struct MyDevice {
    //    pub led: ActorContext<LedMatrix>,
    pub tx: ActorContext<AppTx>,
    pub rx: InterruptContext<AppRx>,
    pub app: ActorContext<App<AppTx>>,
}

impl Device for MyDevice {
    fn mount(&'static self, _: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        //        let display = self.led.mount(timer, supervisor);
        let uart = self.tx.mount((), supervisor);
        let app = self.app.mount(
            AppConfig {
                uart,
                //        display,
                // timer,
            },
            supervisor,
        );

        self.rx.mount(app, supervisor);
    }
}

pub struct AppConfig<U>
where
    U: Uart + 'static,
{
    uart: Address<U>,
    //    display: Address<LedMatrix>,
    // timer: Address<D>,
}

pub struct App<U>
where
    U: Uart + 'static,
{
    uart: Option<Address<U>>,
    // display: Option<Address<LedMatrix>>,
}

impl<U> App<U>
where
    U: Uart + 'static,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            //      display: None,
        }
    }
}
impl<U> Actor for App<U>
where
    U: Uart + 'static,
{
    type Request = SerialData;
    type Response = ();
    type ImmediateFuture = DefaultImmediate<Self>;
    type DeferredFuture = AppLogic<U>;
    type Configuration = AppConfig<U>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config.uart);
        // self.display.replace(config.display);
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

pub struct AppLogic<U>
where
    U: Uart + 'static,
{
    app: Option<App<U>>,
    fut: RequestResponseFuture<U>,
}

impl<U> Unpin for AppLogic<U> where U: Uart + 'static {}

impl<U> Future for AppLogic<U>
where
    U: Uart + 'static,
{
    type Output = (App<U>, ());

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        log::info!("Polling future!");
        let fut = Pin::new(&mut self.fut);
        match fut.poll(cx) {
            Poll::Ready(val) => {
                log::info!("App logic polled future, got val: {:?}", val);
                Poll::Ready((self.app.take().unwrap(), ()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
