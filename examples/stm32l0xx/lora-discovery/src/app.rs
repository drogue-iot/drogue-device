use core::future::Future;
use core::pin::Pin;
use drogue_device::{actors::button::*, traits::lora::*, *};
pub enum Command {
    Send,
}

impl<D: LoraDriver> FromButtonEvent<Command> for App<D> {
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
}

pub struct App<D: LoraDriver> {
    config: Option<LoraConfig>,
    lora: D,
}

impl<D: LoraDriver> App<D> {
    pub fn new(lora: D, config: LoraConfig) -> Self {
        Self {
            config: Some(config),
            lora,
        }
    }
}

impl<D: LoraDriver> Unpin for App<D> {}

impl<D: LoraDriver> Actor for App<D> {
    type Configuration = ();
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
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
        }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match *message {
                Command::Send => {
                    log::info!("Sending message..");
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
