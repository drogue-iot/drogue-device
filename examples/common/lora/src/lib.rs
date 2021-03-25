#![no_std]
use drogue_device::{api::lora::*, driver::button::*, prelude::*};
pub struct App<L>
where
    L: LoraDriver + 'static,
{
    driver: Option<Address<L>>,
    config: LoraConfig,
}

impl<L> App<L>
where
    L: LoraDriver,
{
    pub fn new(config: LoraConfig) -> Self {
        Self {
            driver: None,
            config,
        }
    }
}

impl<L> Actor for App<L>
where
    L: LoraDriver,
{
    type Configuration = Address<L>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.driver.replace(config);
    }

    fn on_start(self) -> Completion<Self> {
        log::info!("Starting app!");
        Completion::defer(async move {
            let driver = self.driver.as_ref().unwrap();
            log::info!("[{}] Configuring LoRa driver", ActorInfo::name());
            driver
                .configure(&self.config)
                .await
                .expect("Error configuring driver");

            log::info!("[{}] Joining LoRaWAN network", ActorInfo::name());
            driver
                .join(ConnectMode::OTAA)
                .await
                .expect("Error joining LoRa Network");

            log::info!(
                "[{}] LoRaWAN network joined successfully",
                ActorInfo::name()
            );
            self
        })
    }
}

impl<L> NotifyHandler<ButtonEvent> for App<L>
where
    L: LoraDriver,
{
    fn on_notify(self, message: ButtonEvent) -> Completion<Self> {
        Completion::defer(async move {
            match message {
                ButtonEvent::Pressed => {
                    log::info!("[{}] button pressed, sending data", ActorInfo::name());
                    let driver = self.driver.as_ref().unwrap();

                    let mut rx_buf = [0; 64];
                    let mut buf = [0; 4];

                    let motd = "Ping".as_bytes();
                    buf[..motd.len()].clone_from_slice(motd);
                    let rxed = driver
                        .send_recv(QoS::Confirmed, 1, motd, &mut rx_buf[..])
                        .await
                        .expect("error sending data");
                    log::info!("[{}] data successfully sent!", ActorInfo::name());
                    if rxed > 0 {
                        log::info!(
                            "[{}] received {} bytes: {:?}",
                            ActorInfo::name(),
                            rxed,
                            &rx_buf[..rxed]
                        );
                    }
                }
                _ => {}
            }
            self
        })
    }
}
