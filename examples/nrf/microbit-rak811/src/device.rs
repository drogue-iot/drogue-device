use drogue_device::{
    driver::{
        gpiote::nrf::*,
        lora::*,
        memory::{Memory, Query},
        timer::Timer,
        uart::dma::Uart,
    },
    hal::timer::nrf::Timer as HalTimer,
    hal::uart::nrf::Uarte as DmaUart,
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::TIMER0;
use nrf52833_hal as hal;

pub type Button = GpioteChannel<LoraDevice, Pin<Input<PullUp>>>;
pub type AppLora = rak811::Rak811<DmaUart<hal::pac::UARTE0>, Pin<Output<PushPull>>>;

pub struct LoraDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<Button>,
    pub btn_send: ActorContext<Button>,
    pub memory: ActorContext<Memory>,
    pub uart: Uart<DmaUart<hal::pac::UARTE0>>,
    pub lora: ActorContext<AppLora>,
    pub timer: Timer<HalTimer<TIMER0>>,
    pub app: ActorContext<App>,
}

impl Device for LoraDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_connect.mount(config.event_bus, supervisor);
        self.btn_send.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount((), supervisor);
        let lora = self.lora.mount(uart, supervisor);
        self.app.mount(lora, supervisor);
    }
}

impl EventHandler<GpioteEvent> for LoraDevice {
    fn on_event(&'static self, event: GpioteEvent) {
        self.btn_send.address().notify(event);
        self.btn_connect.address().notify(event);
    }
}

impl EventHandler<PinEvent> for LoraDevice {
    fn on_event(&'static self, event: PinEvent) {
        match event {
            PinEvent(Channel::Channel0, _) => {
                self.memory.address().notify(Query);
                self.app.address().notify(Join);
            }
            PinEvent(Channel::Channel1, _) => {
                self.app.address().notify(Send);
            }
            _ => {}
        }
    }
}

pub struct App {
    driver: Option<Address<AppLora>>,
    config: LoraConfig,
}

impl App {
    pub fn new(config: LoraConfig) -> Self {
        Self {
            driver: None,
            config,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Initialize;

#[derive(Clone, Debug)]
pub struct Join;

#[derive(Clone, Debug)]
pub struct Send;

impl Actor for App {
    type Configuration = Address<AppLora>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        log::info!("Bound lora");
        self.driver.replace(config);
    }
}

impl NotifyHandler<Join> for App {
    fn on_notify(self, _: Join) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();
            log::info!("Initializing driver");
            driver
                .initialize()
                .await
                .expect("Error initializing driver");

            log::info!("Configuring driver");
            driver
                .configure(&self.config)
                .await
                .expect("Error configuring driver");

            log::info!("Joining network");
            driver.join().await.expect("Error joining LoRa Network");
            self
        })
    }
}

impl NotifyHandler<Send> for App {
    fn on_notify(self, _: Send) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();

            let mut buf = [0; 16];

            let motd = "Hello".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);
            log::info!("Sending data");
            driver.send(QoS::Confirmed, 1, motd).await.ok();

            self
        })
    }
}
