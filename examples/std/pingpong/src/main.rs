#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use drogue_device::*;
use embassy::time::{with_timeout, Duration, TimeoutError};

pub struct MyDevice {
    pinger: ActorContext<'static, PingPonger>,
    ponger: ActorContext<'static, PingPonger>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    DEVICE.configure(MyDevice {
        pinger: ActorContext::new(PingPonger("pinger", "ping")),
        ponger: ActorContext::new(PingPonger("ponger", "pong")),
    });

    DEVICE
        .mount(|device| async move {
            let ponger = device.ponger.mount(None, spawner);
            device.pinger.mount(Some(ponger), spawner);
        })
        .await;
}

pub struct PingPonger(&'static str, &'static str);

pub enum Message {
    Str(&'static str),
    Register(Address<'static, PingPonger>),
}

impl Actor for PingPonger {
    type Configuration = Option<Address<'static, PingPonger>>;
    type Message<'a> = Message;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        me: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            // Notify ponger we can receive pongs
            if let Some(ponger) = config {
                ponger.notify(Message::Register(me)).unwrap();
            }
            log::info!("[{}] started!", self.0);

            // We need to store the pinger to send a message back
            let mut pinger: Option<Address<'static, PingPonger>> = None;
            loop {
                // Send a ping every 10 seconds
                match with_timeout(Duration::from_secs(2), inbox.next()).await {
                    Ok(r) => match r {
                        Some(mut m) => match *m.message() {
                            Message::Str(message) => {
                                log::info!("[{}]: {}", self.0, message);
                                if let Some(pinger) = pinger {
                                    pinger.notify(Message::Str(self.1)).unwrap();
                                }
                            }
                            Message::Register(p) => {
                                pinger.replace(p);
                            }
                        },
                        _ => {}
                    },
                    Err(TimeoutError) => {
                        if let Some(ponger) = config {
                            ponger.notify(Message::Str(self.1)).unwrap();
                        }
                    }
                }
            }
        }
    }
}
