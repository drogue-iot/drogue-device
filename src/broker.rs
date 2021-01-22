use crate::actor::Actor;
use crate::handler::{Completion, NotificationHandler};
use crate::sink::{MultiSink, Sink};
use core::future::Future;
use heapless::{consts, ArrayLength, Vec};

pub trait Message: Clone {}

pub trait Broker {
    fn send<M: Message>(&self, message: M);
    fn receive<M>(&self) -> Future<Output = M>;
}

pub struct Queue<M: 'static> {
    sink: Sink<M>,
    source: Source<M>,
}

impl<M> Broker for PubSubBroker<M> {
    fn send<M: Clone>(&self, message: M) {
        self.sink.send(message);
    }

    fn receive<M>(&self) -> Future<Output = M> {}
}
