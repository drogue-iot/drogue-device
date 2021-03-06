use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    traits::lora::*,
    Actor, Address,
};
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
    config: LoraConfig,
    driver: Option<D>,
}

impl<D: LoraDriver> App<D> {
    pub fn new(config: LoraConfig) -> Self {
        Self {
            config,
            driver: None,
        }
    }
}

impl<D: LoraDriver> Unpin for App<D> {}

impl<D: LoraDriver> Actor for App<D> {
    type Configuration = D;
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, _: Address<'static, Self>, config: Self::Configuration) {
        self.driver.replace(config);
    }

    fn on_start<'m>(self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        let me = unsafe { self.get_unchecked_mut() };
        async move {
            let driver = me.driver.as_mut().unwrap();
            log::info!("Configuring modem");
            driver.configure(&me.config).await.unwrap();
            log::info!("Joining LoRaWAN network");
            driver.join(ConnectMode::OTAA).await.unwrap();
            log::info!("LoRaWAN network joined");
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        let me = unsafe { self.get_unchecked_mut() };
        async move {
            let driver = me.driver.as_mut().unwrap();
            match message {
                Command::Send => {
                    log::info!("Sending data..");
                    let result = driver.send(QoS::Confirmed, 1, b"ping").await;
                    log::info!("Data sent: {:?}", result);
                }
            }
        }
    }
}
