use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;
use drogue_device::{actors::button::*, drivers::led::*, traits::lora::*, *};
use embedded_hal::digital::v2::{StatefulOutputPin, ToggleableOutputPin};
use heapless::String;

#[derive(Clone, Copy)]
pub enum Command {
    Tick,
    Send,
    TickAndSend,
}

impl<D, L1, L2, L3, L4> FromButtonEvent<Command> for App<D, L1, L2, L3, L4>
where
    D: LoraDriver,
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
    L2: StatefulOutputPin + ToggleableOutputPin + 'static,
    L3: StatefulOutputPin + ToggleableOutputPin + 'static,
    L4: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::TickAndSend),
        }
    }
}

pub struct AppConfig<D>
where
    D: LoraDriver + 'static,
{
    pub lora: D,
}

pub struct AppInitConfig<L1, L2, L3, L4>
where
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
    L2: StatefulOutputPin + ToggleableOutputPin + 'static,
    L3: StatefulOutputPin + ToggleableOutputPin + 'static,
    L4: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    pub lora: LoraConfig,
    pub init_led: Led<L1>,
    pub tx_led: Led<L2>,
    pub user_led: Led<L3>,
    pub green_led: Led<L4>,
}

pub struct App<D, L1, L2, L3, L4>
where
    D: LoraDriver + 'static,
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
    L2: StatefulOutputPin + ToggleableOutputPin + 'static,
    L3: StatefulOutputPin + ToggleableOutputPin + 'static,
    L4: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    config: AppInitConfig<L1, L2, L3, L4>,
    cfg: Option<AppConfig<D>>,
    counter: usize,
}

impl<D, L1, L2, L3, L4> App<D, L1, L2, L3, L4>
where
    D: LoraDriver,
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
    L2: StatefulOutputPin + ToggleableOutputPin + 'static,
    L3: StatefulOutputPin + ToggleableOutputPin + 'static,
    L4: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    pub fn new(config: AppInitConfig<L1, L2, L3, L4>) -> Self {
        Self {
            config,
            cfg: None,
            counter: 0,
        }
    }

    fn tick(&mut self) {
        self.counter += 1;
        log::info!("Ticked: {}", self.counter);
    }

    async fn send(&mut self) {
        log::info!("Sending message...");
        self.config.tx_led.on().ok();

        if let Some(ref mut cfg) = &mut self.cfg {
            let mut tx = String::<heapless::consts::U32>::new();
            write!(&mut tx, "ping:{}", self.counter).ok();
            log::info!("Message: {}", &tx);
            let tx = tx.into_bytes();

            let mut rx = [0; 64];
            let result = cfg.lora.send_recv(QoS::Confirmed, 1, &tx, &mut rx).await;

            match result {
                Ok(rx_len) => {
                    log::info!("Message sent!");
                    if rx_len > 0 {
                        let response = &rx[0..rx_len];
                        match core::str::from_utf8(response) {
                            Ok(str) => {
                                log::info!("Received {} bytes from uplink:\n{}", rx_len, str)
                            }
                            Err(_) => log::info!(
                                "Received {} bytes from uplink: {:x?}",
                                rx_len,
                                &rx[0..rx_len]
                            ),
                        }
                        match response {
                            b"led:on" => {
                                self.config.user_led.on().ok();
                            }
                            b"led:off" => {
                                self.config.user_led.off().ok();
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error sending message: {:?}", e);
                }
            }
        }

        self.config.tx_led.off().ok();
    }
}

impl<D, L1, L2, L3, L4> Unpin for App<D, L1, L2, L3, L4>
where
    D: LoraDriver,
    L1: StatefulOutputPin + ToggleableOutputPin,
    L2: StatefulOutputPin + ToggleableOutputPin,
    L3: StatefulOutputPin + ToggleableOutputPin,
    L4: StatefulOutputPin + ToggleableOutputPin,
{
}

impl<D, L1, L2, L3, L4> Actor for App<D, L1, L2, L3, L4>
where
    D: LoraDriver + 'static,
    L1: StatefulOutputPin + ToggleableOutputPin + 'static,
    L2: StatefulOutputPin + ToggleableOutputPin + 'static,
    L3: StatefulOutputPin + ToggleableOutputPin + 'static,
    L4: StatefulOutputPin + ToggleableOutputPin + 'static,
{
    #[rustfmt::skip]
    type Configuration = AppConfig<D>;
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.cfg.replace(config);
    }

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            self.config.init_led.on().ok();
            if let Some(mut cfg) = self.cfg.take() {
                cfg.lora
                    .configure(&self.config.lora)
                    .await
                    .expect("error configuring lora");
                log::info!("Lora driver configured");
                cfg.lora
                    .join(ConnectMode::OTAA)
                    .await
                    .expect("error joining lora network");
                log::info!("Lora network joined");
                self.cfg.replace(cfg);
            }
            self.config.init_led.off().ok();
        }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match message {
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
            }
        }
    }
}
