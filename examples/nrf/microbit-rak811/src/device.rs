use drogue_device::{
    api::{delayer::*, lora::*, scheduler::*, uart::*},
    driver::{
        lora::*,
        memory::{Memory, Query},
        timer::*,
        uart::dma::DmaUart,
        uart::*,
    },
    port::nrf::{gpiote::*, timer::Timer as HalTimer, uarte::Uarte as HalUart},
    prelude::*,
};
use hal::gpio::{Input, Output, Pin, PullUp, PushPull};
use hal::pac::{TIMER0, UARTE0};
use heapless::consts;

use nrf52833_hal as hal;

pub type AppTimer = Timer<HalTimer<TIMER0>>;
pub type AppUart = DmaUart<HalUart<UARTE0>, <AppTimer as Package>::Primary, consts::U64>;
pub type Rak811Lora = rak811::Rak811<
    <AppUart as Package>::Primary,
    <AppTimer as Package>::Primary,
    Pin<Output<PushPull>>,
>;
pub type Button = GpioteChannel<LoraDevice, Pin<Input<PullUp>>>;
pub type AppLora = <Rak811Lora as Package>::Primary;

pub struct LoraDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<Button>,
    pub btn_send: ActorContext<Button>,
    pub memory: ActorContext<Memory>,
    pub uart: AppUart,
    pub lora: Rak811Lora,
    pub timer: AppTimer,
    pub app: ActorContext<App<AppLora>>,
}

impl Device for LoraDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        self.memory.mount((), supervisor);
        self.gpiote.mount(config.event_bus, supervisor);
        self.btn_connect.mount(config.event_bus, supervisor);
        self.btn_send.mount(config.event_bus, supervisor);
        let timer = self.timer.mount((), supervisor);
        let uart = self.uart.mount(timer, supervisor);
        let lora = self.lora.mount((uart, timer), supervisor);
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
            PinEvent(Channel::Channel0, PinState::Low) => {
                self.memory.address().notify(Query);
                self.app.address().notify(Join);
            }
            PinEvent(Channel::Channel1, PinState::Low) => {
                self.memory.address().notify(Query);
                self.app.address().notify(Send);
            }
            _ => {}
        }
    }
}

pub struct App<L>
where
    L: LoraDriver + 'static,
{
    driver: Option<Address<L>>,
    config: LoraConfig,
}

impl<L> App<L>
where
    L: LoraDriver,
{
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

impl<L> Actor for App<L>
where
    L: LoraDriver,
{
    type Configuration = Address<L>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        log::info!("Bound lora");
        self.driver.replace(config);
    }
}

impl<L> NotifyHandler<Join> for App<L>
where
    L: LoraDriver,
{
    fn on_notify(self, _: Join) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();
            log::info!("Configuring driver");
            driver
                .configure(&self.config)
                .await
                .expect("Error configuring driver");

            log::info!("Joining network");
            driver
                .join(ConnectMode::OTAA)
                .await
                .expect("Error joining LoRa Network");

            log::info!("Network joined");
            self
        })
    }
}

impl<L> NotifyHandler<Send> for App<L>
where
    L: LoraDriver,
{
    fn on_notify(self, _: Send) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();

            let mut buf = [0; 16];

            let motd = "Hello".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);
            log::info!("Sending data");
            driver.send(QoS::Confirmed, 1, motd).await.ok();
            log::info!("Data sent!");

            self
        })
    }
}
