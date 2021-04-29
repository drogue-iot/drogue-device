use crate::lora::*;
use core::future::Future;
use core::pin::Pin;
use drogue_device::actors::led::{Led, LedMessage};
use drogue_device::{actors::button::*, traits::lora::*, *};
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

pub struct AppConfig<'a, D, P1, P2, P3, P4>
where
    D: LoraDriver + 'a,
    P1: OutputPin + 'a,
    P2: OutputPin + 'a,
    P3: OutputPin + 'a,
    P4: OutputPin + 'a,
{
    // lora actor
    pub lora: Address<'a, LoraActor<D>>,
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
    D: LoraDriver + 'static,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    config: Option<LoraConfig>,
    cfg: Option<AppConfig<'static, D, P1, P2, P3, P4>>,
}

impl<D, P1, P2, P3, P4> App<D, P1, P2, P3, P4>
where
    D: LoraDriver,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    pub fn new(config: LoraConfig) -> Self {
        Self {
            config: Some(config),
            cfg: None,
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
{
}

impl<D, P1, P2, P3, P4> Actor for App<D, P1, P2, P3, P4>
where
    D: LoraDriver + 'static,
    P1: OutputPin + 'static,
    P2: OutputPin + 'static,
    P3: OutputPin + 'static,
    P4: OutputPin + 'static,
{
    #[rustfmt::skip]
    type Configuration = AppConfig<'static, D, P1, P2, P3, P4>;
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type Response<'m> where D: 'm = ();
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.cfg.replace(config);
    }

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            log_stack!();
            let config = self.config.take().unwrap();
            if let Some(cfg) = &self.cfg {
                cfg.led4.notify(LedMessage::On).await;
                cfg.lora.request(LoraCommand::Configure(&config)).await;
                cfg.lora.request(LoraCommand::Join).await;
                cfg.led4.notify(LedMessage::Off).await;
            }
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            log_stack!();
            match message {
                Command::Send => {
                    if let Some(cfg) = &self.cfg {
                        log::info!("Sending message...");
                        cfg.led1.notify(LedMessage::On).await;
                        let mut rx = [0; 255];
                        let result = cfg
                            .lora
                            .request(LoraCommand::SendRecv("ping".as_bytes(), &mut rx))
                            .await;

                        cfg.led1.notify(LedMessage::Off).await;
                        match result {
                            LoraResult::OkSent(rx_len) => {
                                log::info!("Message sent!");
                                let response = &rx[0..rx_len];
                                match core::str::from_utf8(response) {
                                    Ok(str) => log::info!("Received from uplink:\n{}", str),
                                    Err(_) => log::info!(
                                        "Received {} bytes from uplink: {:x?}",
                                        rx_len,
                                        &rx[0..rx_len]
                                    ),
                                }
                                match response {
                                    b"led:on" => cfg.led3.notify(LedMessage::On).await,
                                    b"led:off" => cfg.led3.notify(LedMessage::Off).await,
                                    _ => {}
                                }
                            }
                            LoraResult::Err(e) => {
                                log::error!("Error sending message: {:?}", e);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
