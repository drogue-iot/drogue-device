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
    driver: D,
}

impl<D: LoraDriver> App<D> {
    pub fn new(join_mode: JoinMode, driver: D) -> Self {
        Self { join_mode, driver }
    }
}

impl<D: LoraDriver> Actor for App<D> {
    type Message<'m> = Command
    where
        D: 'm;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        D: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            defmt::info!("Joining LoRaWAN network");
            self.driver.join(self.join_mode).await.unwrap();
            defmt::info!("LoRaWAN network joined");
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Send => {
                            defmt::info!("Sending data..");
                            let result = self.driver.send(QoS::Confirmed, 1, b"ping").await;
                            defmt::info!("Data sent: {:?}", result);
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}
