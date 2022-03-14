use core::future::Future;
use drogue_device::{traits::lora::*, Actor, Address, Inbox};
pub enum AppCommand {
    Send,
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
    type Message<'m> = AppCommand
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
            log::info!("Joining LoRaWAN network");
            self.driver.join(self.join_mode).await.unwrap();
            log::info!("LoRaWAN network joined");
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        AppCommand::Send => {
                            log::info!("Sending data..");
                            let result = self.driver.send(QoS::Confirmed, 1, b"ping").await;
                            log::info!("Data sent: {:?}", result);
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}
