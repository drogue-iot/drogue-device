use core::fmt::Write;
use core::future::Future;
use drogue_device::{
    traits::led::*,
    traits::lora::{ConnectMode, LoraConfig, LoraDriver, QoS},
    Actor, Address, Inbox,
};
use embassy::time::{Duration, Timer};
use heapless::String;

pub struct App<D, LED>
where
    D: LoraDriver + 'static,
    LED: Led + 'static,
{
    lora: LoraConfig,
    led: LED,
    driver: Option<D>,
    counter: usize,
    interval: Duration,
}

impl<D, LED> App<D, LED>
where
    D: LoraDriver,
    LED: Led + 'static,
{
    pub fn new(config: LoraConfig, led: LED, interval: Duration) -> Self {
        Self {
            lora: config,
            led,
            driver: None,
            counter: 0,
            interval,
        }
    }

    async fn send(&mut self) {
        defmt::info!("Sending message...");
        self.led.on().ok();

        if let Some(ref mut driver) = &mut self.driver {
            let mut tx = String::<32>::new();
            write!(&mut tx, "ping:{}", self.counter).ok();
            defmt::info!("Message: {}", &tx.as_str());
            let tx = tx.into_bytes();

            let result = driver.send(QoS::Unconfirmed, 1, &tx).await;

            match result {
                Ok(_) => {
                    defmt::info!("Message sent!");
                }
                Err(e) => {
                    defmt::error!("Error sending message: {:?}", e);
                }
            }
        }
        self.led.off().ok();
    }
}

impl<D, LED> Actor for App<D, LED>
where
    D: LoraDriver + 'static,
    LED: Led + 'static,
{
    #[rustfmt::skip]
    type Configuration = D;
    #[rustfmt::skip]
    type OnMountFuture<'m, M> where D: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.driver.replace(config);
        async move {
            defmt::info!("Configuring");
            self.led.on().ok();
            let driver = self.driver.as_mut().unwrap();
            if let Err(e) = driver.configure(&self.lora).await {
                defmt::error!("Error configuring lora driver: {}", e);
                panic!();
            }
            defmt::trace!("Lora driver configured");
            driver
                .join(ConnectMode::OTAA)
                .await
                .expect("error joining lora network");
            Timer::after(Duration::from_millis(500)).await;
            self.led.off().ok();
            defmt::info!("Configuration finished");
            loop {
                Timer::after(self.interval).await;
                self.send().await;
                self.counter += 1;
            }
        }
    }
}
