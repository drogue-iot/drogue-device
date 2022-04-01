#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_actor::*;
use embassy::time::{Duration, Ticker};
use futures::{
    future::{select, Either},
    pin_mut, StreamExt,
};

#[embassy::main]
async fn main(s: embassy::executor::Spawner) {
    // Example of circular references
    static PINGER: ActorContext<Pinger> = ActorContext::new();
    static PONGER: ActorContext<Ponger> = ActorContext::new();

    let pinger = PINGER.address();
    let ponger = PONGER.address();

    PINGER.mount(s, Pinger(ponger));
    PONGER.mount(s, Ponger(pinger));
}

#[derive(Debug)]
pub struct Ping;

#[derive(Debug)]
pub struct Pong;

pub struct Pinger(Address<Ping>);
pub struct Ponger(Address<Pong>);

#[actor]
impl Actor for Pinger {
    type Message<'m> = Pong;
    async fn on_mount<M>(&mut self, _: Address<Pong>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        println!("Pinger started!");

        let mut ticker = Ticker::every(Duration::from_secs(2));
        // We need to store the pinger to send a message back
        loop {
            let next = inbox.next();
            let tick = ticker.next();

            pin_mut!(next);
            pin_mut!(tick);

            // Send a ping every 10 seconds
            match select(next, tick).await {
                Either::Left((m, _)) => {
                    println!("{:?}", m);
                }
                Either::Right((_, _)) => {
                    self.0.notify(Ping).await;
                }
            }
        }
    }
}

#[actor]
impl Actor for Ponger {
    type Message<'m> = Ping;
    async fn on_mount<M>(&mut self, _: Address<Ping>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        println!("Ponger started!");

        loop {
            // Send a ping every 10 seconds
            let m = inbox.next().await;
            println!("{:?}", m);
            self.0.notify(Pong).await;
        }
    }
}
