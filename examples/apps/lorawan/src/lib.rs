#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

use core::fmt::Write;
use core::future::Future;
use drogue_device::{
    actors,
    traits::{
        button::Button,
        led::Led,
        lora::{JoinMode, LoraDriver, QoS},
    },
    Actor, ActorContext, Address, Inbox, *,
};
use embassy::executor::Spawner;
use heapless::String;

pub trait LoraBoard
where
    Self: 'static,
{
    type JoinLed: Led;
    type TxLed: Led;
    type CommandLed: Led;
    type SendTrigger: SendTrigger;
    type Driver: LoraDriver;
}

pub trait SendTrigger {
    type TriggerFuture<'m>: Future
    where
        Self: 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m>;
}

#[derive(Clone, Copy)]
pub enum Command {
    Tick,
    Send,
    TickAndSend,
}

impl<B> SendTrigger for B
where
    B: Button + 'static,
{
    type TriggerFuture<'m>
    where
        B: 'm,
    = impl Future + 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        self.wait_released()
    }
}

pub struct LoraDeviceConfig<B>
where
    B: LoraBoard + 'static,
{
    pub join_led: Option<B::JoinLed>,
    pub tx_led: Option<B::TxLed>,
    pub command_led: Option<B::CommandLed>,
    pub send_trigger: B::SendTrigger,
    pub driver: B::Driver,
}

pub struct LoraDevice<B>
where
    B: LoraBoard + 'static,
{
    driver: ActorContext<actors::lora::LoraActor<B::Driver>>,
    trigger: ActorContext<AppTrigger<B>>,
    app: ActorContext<App<B>>,
}

impl<B> LoraDevice<B>
where
    B: LoraBoard + 'static,
{
    pub fn new() -> Self {
        Self {
            driver: ActorContext::new(),
            trigger: ActorContext::new(),
            app: ActorContext::new(),
        }
    }

    pub async fn mount(&'static self, spawner: Spawner, config: LoraDeviceConfig<B>) {
        let driver = self
            .driver
            .mount(spawner, actors::lora::LoraActor::new(config.driver));
        let app = self.app.mount(
            spawner,
            App::new(config.join_led, config.tx_led, config.command_led, driver),
        );

        self.trigger.mount(
            spawner,
            AppTrigger {
                trigger: config.send_trigger,
                app,
            },
        );
    }
}

pub struct App<B>
where
    B: LoraBoard + 'static,
{
    counter: usize,
    join_led: Option<B::JoinLed>,
    tx_led: Option<B::TxLed>,
    command_led: Option<B::CommandLed>,
    driver: Address<actors::lora::LoraActor<B::Driver>>,
}

impl<B> App<B>
where
    B: LoraBoard + 'static,
{
    pub fn new(
        join_led: Option<B::JoinLed>,
        tx_led: Option<B::TxLed>,
        command_led: Option<B::CommandLed>,
        driver: Address<actors::lora::LoraActor<B::Driver>>,
    ) -> Self {
        Self {
            join_led,
            tx_led,
            command_led,
            counter: 0,
            driver,
        }
    }

    fn tick(&mut self) {
        self.counter += 1;
        defmt::info!("Ticked: {}", self.counter);
    }

    async fn send(&mut self) {
        defmt::info!("Sending message...");
        self.tx_led.as_mut().map(|l| l.on().ok());

        let mut tx = String::<32>::new();
        write!(&mut tx, "ping:{}", self.counter).ok();
        defmt::info!("Message: {}", &tx.as_str());
        let tx = tx.into_bytes();

        let mut rx = [0; 64];
        let result = self.driver.send_recv(QoS::Confirmed, 1, &tx, &mut rx).await;

        match result {
            Ok(rx_len) => {
                defmt::info!("Message sent!");
                if rx_len > 0 {
                    let response = &rx[0..rx_len];
                    match core::str::from_utf8(response) {
                        Ok(str) => {
                            defmt::info!("Received {} bytes from uplink:\n{}", rx_len, str)
                        }
                        Err(_) => defmt::info!(
                            "Received {} bytes from uplink: {:x}",
                            rx_len,
                            &rx[0..rx_len]
                        ),
                    }
                    match response {
                        b"led:on" => {
                            self.command_led.as_mut().map(|l| l.on().ok());
                        }
                        b"led:off" => {
                            self.command_led.as_mut().map(|l| l.off().ok());
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                defmt::error!("Error sending message: {:?}", e);
            }
        }

        self.tx_led.as_mut().map(|l| l.off().ok());
    }
}

impl<B> Actor for App<B>
where
    B: LoraBoard + 'static,
{
    type Message<'m>
    where
        B: 'm,
    = Command;

    type OnMountFuture<'m, M>
    where
        B: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let join_mode = JoinMode::OTAA {
                dev_eui: DEV_EUI.trim_end().into(),
                app_eui: APP_EUI.trim_end().into(),
                app_key: APP_KEY.trim_end().into(),
            };
            self.join_led.as_mut().map(|l| l.on().ok());
            defmt::info!("Joining LoRaWAN network");
            self.driver
                .join(join_mode)
                .await
                .expect("error joining lora network");
            defmt::info!("LoRaWAN network joined");
            self.join_led.as_mut().map(|l| l.off().ok());
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Tick => {
                            self.tick();
                        }
                        Command::Send => {
                            self.send().await;
                        }
                        Command::TickAndSend => {
                            self.tick();
                            self.send().await;
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

pub struct AppTrigger<B>
where
    B: LoraBoard + 'static,
{
    trigger: B::SendTrigger,
    app: Address<App<B>>,
}

impl<B> Actor for AppTrigger<B>
where
    B: LoraBoard + 'static,
{
    type OnMountFuture<'m, M>
    where
        Self: 'm,
        B: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                self.trigger.wait().await;
                let _ = self.app.request(Command::TickAndSend).unwrap().await;
            }
        }
    }
}

pub struct TimeTrigger(pub embassy::time::Duration);
impl SendTrigger for TimeTrigger {
    type TriggerFuture<'m>
    where
        Self: 'm,
    = impl Future + 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        embassy::time::Timer::after(self.0)
    }
}

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");
