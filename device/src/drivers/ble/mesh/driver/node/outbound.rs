use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::driver::node::NodeMutex;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::model::ModelIdentifier;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, AccessPayload};
use core::marker::PhantomData;
use embassy_util::channel::mpmc::{Channel, Sender};

use embassy_util::{select, Either};

const MAX_MESSAGE: usize = 1;

pub(crate) struct OutboundAccessChannel<'a> {
    channel: Channel<NodeMutex, AccessMessage, MAX_MESSAGE>,
    _a: PhantomData<&'a ()>,
}

impl<'a> OutboundAccessChannel<'a> {
    fn new() -> Self {
        Self {
            channel: Channel::new(),
            _a: PhantomData,
        }
    }

    pub(crate) async fn send(&self, message: AccessMessage) {
        self.channel.send(message).await;
    }

    async fn next(&self) -> AccessMessage {
        self.channel.recv().await
    }

    pub(crate) fn sender(&'a self) -> Sender<'a, NodeMutex, AccessMessage, MAX_MESSAGE> {
        self.channel.sender()
    }
}

// --

#[derive(Clone)]
pub struct OutboundPublishMessage {
    pub(crate) element_address: UnicastAddress,
    pub(crate) model_identifier: ModelIdentifier,
    pub(crate) payload: AccessPayload,
}

impl OutboundPublishMessage {
    pub fn model_key(&self) -> ModelKey {
        ModelKey::new(self.element_address, self.model_identifier)
    }
}

pub(crate) struct OutboundPublishChannel<'a> {
    channel: Channel<NodeMutex, OutboundPublishMessage, MAX_MESSAGE>,
    _a: PhantomData<&'a ()>,
}

impl<'a> OutboundPublishChannel<'a> {
    fn new() -> Self {
        Self {
            channel: Channel::new(),
            _a: PhantomData,
        }
    }

    pub(crate) async fn send(&self, message: OutboundPublishMessage) {
        self.channel.send(message).await;
    }

    async fn next(&self) -> OutboundPublishMessage {
        self.channel.recv().await
    }

    pub(crate) fn sender(&'a self) -> Sender<'a, NodeMutex, OutboundPublishMessage, MAX_MESSAGE> {
        self.channel.sender()
    }
}
// --

pub struct Outbound<'a> {
    pub(crate) access: OutboundAccessChannel<'a>,
    pub(crate) publish: OutboundPublishChannel<'a>,
}

impl<'a> Default for Outbound<'a> {
    fn default() -> Self {
        Self {
            access: OutboundAccessChannel::new(),
            publish: OutboundPublishChannel::new(),
        }
    }
}

pub enum OutboundEvent {
    Access(AccessMessage),
    Publish(OutboundPublishMessage),
}

impl<'a> Outbound<'a> {
    pub async fn next(&self) -> OutboundEvent {
        let access_fut = self.access.next();
        let publish_fut = self.publish.next();

        match select(access_fut, publish_fut).await {
            Either::First(access) => OutboundEvent::Access(access),
            Either::Second(publish) => OutboundEvent::Publish(publish),
        }
    }
}
