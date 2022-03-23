#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::*;
use embassy::time::{with_timeout, Duration, TimeoutError};

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let ponger = spawn_actor!(
        spawner,
        PONGER,
        PingPonger,
        PingPonger("ponger", "pong", None)
    );

    spawn_actor!(
        spawner,
        PINGER,
        PingPonger,
        PingPonger("pinger", "ping", Some(ponger))
    );
}

pub struct PingPonger(&'static str, &'static str, Option<Address<PingPonger>>);

pub enum Message {
    Str(&'static str),
    Register(Address<PingPonger>),
}

#[actor]
impl Actor for PingPonger {
    type Message<'a> = Message;
    async fn on_mount<M>(&mut self, me: Address<Self>, inbox: &mut M)
    where
        M: Inbox<Self>,
    {
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
