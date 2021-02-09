use drogue_device::{
    driver::{gpiote::nrf::*, lora::*, uart::Uart},
    hal::uart::nrf::Uarte as HalUart,
    prelude::*,
};
use hal::gpio::{Input, Pin, PullUp};
use nrf52833_hal as hal;

pub type Button = GpioteChannel<LoraDevice, Pin<Input<PullUp>>>;
pub type AppLora = rak811::Rak811<HalUart<hal::pac::UARTE0>>;

pub struct LoraDevice {
    pub gpiote: InterruptContext<Gpiote<Self>>,
    pub btn_connect: ActorContext<Button>,
    pub btn_send: ActorContext<Button>,
    pub uart: Uart<HalUart<hal::pac::UARTE0>>,
    pub lora: ActorContext<AppLora>,
    pub app: ActorContext<App>,
}

impl Device for LoraDevice {
    fn mount(&'static self, bus: Address<EventBus<Self>>, supervisor: &mut Supervisor) {
        self.gpiote.mount(supervisor);
        self.btn_connect.mount(supervisor);
        self.btn_send.mount(supervisor);
        let uart = self.uart.mount(bus, supervisor);
        let lora = self.lora.mount(supervisor);
        self.app.mount(supervisor);

        self.gpiote.configure(bus);
        self.btn_connect.configure(bus);
        self.btn_send.configure(bus);
        self.lora.configure(uart);
        self.app.configure(lora);
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
    config: LoraConfig<'static>,
}

impl App {
    pub fn new(config: LoraConfig<'static>) -> Self {
        Self {
            driver: None,
            config,
        }
    }
}
impl Actor for App {}

#[derive(Clone, Debug)]
pub struct Initialize;

#[derive(Clone, Debug)]
pub struct Join;

#[derive(Clone, Debug)]
pub struct Send;

impl Configurable for App {
    type Configuration = Address<AppLora>;
    fn configure(&mut self, config: Self::Configuration) {
        log::info!("Bound lora");
        self.driver.replace(config);
    }
}

impl NotifyHandler<Join> for App {
    fn on_notify(self, _: Join) -> Completion<Self> {
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();

            log::info!("Configuring driver");
            driver.configure(&self.config).await.unwrap();

            log::info!("Joining network");
            driver.join().await.unwrap();
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
            driver.send(QoS::Confirmed, 1, motd).await.unwrap();

            self
        })
    }
}
