use core::fmt::Write;
use core::future::Future;
use drogue_device::{
    actors::button::*,
    traits::led::*,
    traits::lora::{JoinMode, LoraDriver, QoS},
    Actor, Address, Inbox,
};
use embassy::time::{Duration, Timer};
use heapless::String;

#[derive(Clone, Copy)]
pub enum Command {
    Tick,
    Send,
    TickAndSend,
}

impl<D, LED1, LED2, LED3> FromButtonEvent<Command> for App<D, LED1, LED2, LED3>
where
    D: LoraDriver,
    LED1: Led + 'static,
    LED2: Led + 'static,
    LED3: Led + 'static,
{
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::TickAndSend),
        }
    }
}

pub struct App<D, LED1, LED2, LED3>
where
    D: LoraDriver + 'static,
    LED1: Led + 'static,
    LED2: Led + 'static,
    LED3: Led + 'static,
{
    join_mode: JoinMode,
    init_led: LED1,
    tx_led: LED2,
    user_led: LED3,
    driver: Option<D>,
    counter: usize,
}

impl<D, LED1, LED2, LED3> App<D, LED1, LED2, LED3>
where
    D: LoraDriver,
    LED1: Led + 'static,
    LED2: Led + 'static,
    LED3: Led + 'static,
{
    pub fn new(join_mode: JoinMode, init_led: LED1, tx_led: LED2, user_led: LED3) -> Self {
        Self {
            join_mode,
            init_led,
            tx_led,
            user_led,
            driver: None,
            counter: 0,
        }
    }

    fn tick(&mut self) {
        self.counter += 1;
        defmt::info!("Ticked: {}", self.counter);
    }

    async fn send(&mut self) {
        defmt::info!("Sending message...");
        self.tx_led.on().ok();

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
        Timer::after(Duration::from_secs(1)).await;
        self.tx_led.off().ok();
    }
}

impl<D, LED1, LED2, LED3> Unpin for App<D, LED1, LED2, LED3>
where
    D: LoraDriver,
    LED1: Led + 'static,
    LED2: Led + 'static,
    LED3: Led + 'static,
{
}

impl<D, LED1, LED2, LED3> Actor for App<D, LED1, LED2, LED3>
where
    D: LoraDriver + 'static,
    LED1: Led + 'static,
    LED2: Led + 'static,
    LED3: Led + 'static,
{
    #[rustfmt::skip]
    type Configuration = D;
    #[rustfmt::skip]
    type Message<'m> where D: 'm = Command;
    #[rustfmt::skip]
    type OnMountFuture<'m, M> where D: 'm, M: 'm = impl Future<Output = ()> + 'm;

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
            self.init_led.on().ok();
            defmt::info!("Joining LoRaWAN network");
            let driver = self.driver.as_mut().unwrap();
            driver
                .join(self.join_mode)
                .await
                .expect("error joining lora network");
            Timer::after(Duration::from_millis(500)).await;
            self.init_led.off().ok();
            defmt::info!("LoRaWAN network joined");
            loop {
                match *inbox.next().await.message() {
                    Command::Tick => {
                        self.tick();
                    }
                    Command::Send => {
                        self.send().await;
                    }
                    Command::TickAndSend => {
                        self.tick();
                        self.send().await;
                    }
                }
            }
        }
    }
}
