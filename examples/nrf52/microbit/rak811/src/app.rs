use core::future::Future;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    traits::lora::*,
    Actor, Address, Inbox,
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
    join_mode: JoinMode,
    driver: Option<D>,
}

impl<D: LoraDriver> App<D> {
    pub fn new(join_mode: JoinMode) -> Self {
        Self {
            join_mode,
            driver: None,
        }
    }
}

impl<D: LoraDriver> Unpin for App<D> {}

impl<D: LoraDriver> Actor for App<D> {
    type Configuration = D;

    type Message<'m>
    where
        D: 'm,
    = Command;

    type OnMountFuture<'m, M>
    where
        D: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.driver.replace(config);
        async move {
            let driver = self.driver.as_mut().unwrap();
            defmt::info!("Joining LoRaWAN network");
            driver.join(self.join_mode).await.unwrap();
            defmt::info!("LoRaWAN network joined");
            let driver = self.driver.as_mut().unwrap();
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Send => {
                            defmt::info!("Sending data..");
                            let result = driver.send(QoS::Confirmed, 1, b"ping").await;
                            defmt::info!("Data sent: {:?}", result);
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}
