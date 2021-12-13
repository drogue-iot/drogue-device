#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use drogue_device::*;
use embassy::time::{with_timeout, Duration, TimeoutError};

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    static PINGER: ActorContext<PingPonger> = ActorContext::new();
    static PONGER: ActorContext<PingPonger> = ActorContext::new();

    let ponger = PONGER.mount(spawner, PingPonger("ponger", "pong", None));
    PINGER.mount(spawner, PingPonger("pinger", "ping", Some(ponger)));
}

pub struct PingPonger(&'static str, &'static str, Option<Address<PingPonger>>);

pub enum Message {
    Str(&'static str),
    Register(Address<PingPonger>),
}

impl Actor for PingPonger {
    type Message<'a> = Message;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        me: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            // Notify ponger we can receive pongs
            if let Some(ponger) = self.2 {
                ponger.notify(Message::Register(me)).unwrap();
            }
            log::info!("[{}] started!", self.0);

            // We need to store the pinger to send a message back
            let mut pinger: Option<Address<PingPonger>> = None;
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
                        if let Some(ponger) = self.2 {
                            ponger.notify(Message::Str(self.1)).unwrap();
                        }
                    }
                }
            }
        }
    }
}
