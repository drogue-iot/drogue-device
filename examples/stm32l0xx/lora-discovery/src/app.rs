use core::future::Future;
use core::pin::Pin;
use drogue_device::{actors::button::*, traits::lora::*, *};
use drogue_device::actors::led::{Led, LedMessage};
use embassy_stm32::system::OutputPin;

pub enum Command {
    Send,
}

impl<D, P1, P2, P3, P4> FromButtonEvent<Command> for App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
}

pub struct AppConfig<'a, P1, P2, P3, P4>
where
    P1: OutputPin + 'a,
    P2: OutputPin + 'a,
    P3: OutputPin + 'a,
    P4: OutputPin + 'a,
{
    // green led
    pub led1: Address<'a, Led<P1>>,
    // green led 2
    pub led2: Address<'a, Led<P2>>,
    // blue led
    pub led3: Address<'a, Led<P3>>,
    // red led
    pub led4: Address<'a, Led<P4>>,
}

pub struct App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    config: Option<LoraConfig>,
    lora: D,
    cfg: Option<AppConfig<'static, P1, P2, P3, P4>>,
}

impl<D, P1, P2, P3, P4> App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    pub fn new(lora: D, config: LoraConfig) -> Self {
        Self {
            config: Some(config),
            cfg: None,
            lora,
        }
    }
}

impl<D, P1, P2, P3, P4> Unpin for App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{}

impl<D, P1, P2, P3, P4> Actor for App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    #[rustfmt::skip]
    type Configuration = AppConfig<'static, P1, P2, P3, P4>;
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

            if let Some(cfg) = &self.cfg {
                cfg.led4.notify(LedMessage::On).await;
            }

            let config = self.config.take().unwrap();
            if let Err(e) = self.lora.configure(&config).await {
                log::error!("Error configuring: {:?}", e);
            } else {
                log::info!("LoRa driver configured");
            }
            self.lora
                .join(ConnectMode::OTAA)
                .await
                .expect("Error joining network");
            if let Err(e) = self.lora.join(ConnectMode::OTAA).await {
                log::error!("Error joining network : {:?}", e);
            } else {
                log::info!("LoRa network joined");
            }

            if let Some(cfg) = &self.cfg {
                cfg.led4.notify(LedMessage::Off).await;
            }
        }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match *message {
                Command::Send => {
                    log::info!("Sending message...");
                    let mut rx = [0; 255];
                    match self
                        .lora
                        .send_recv(QoS::Confirmed, 1, "ping".as_bytes(), &mut rx)
                        .await
                    {
                        Err(e) => {
                            log::error!("Error sending message: {:?}", e);
                        }
                        Ok(len) => {
                            log::info!("Message sent");
                            if len > 0 {
                                log::info!(
                                    "Received {} bytes from uplink: {:x?}",
                                    len,
                                    &rx[0..len]
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
