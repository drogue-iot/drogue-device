use core::future::Future;
use drogue_device::traits::lora::*;
use ector::{Actor, Address, Inbox};
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
    type Message<'m> = AppCommand;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        D: 'm,
        M: 'm + Inbox<Self::Message<'m>>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            log::info!("Joining LoRaWAN network");
            self.driver.join(self.join_mode).await.unwrap();
            log::info!("LoRaWAN network joined");
            loop {
                match inbox.next().await {
                    AppCommand::Send => {
                        log::info!("Sending data..");
                        let result = self.driver.send(QoS::Confirmed, 1, b"ping").await;
                        log::info!("Data sent: {:?}", result);
                    }
                }
            }
        }
    }
}
