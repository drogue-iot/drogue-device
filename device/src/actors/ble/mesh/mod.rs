pub mod bearer;

use crate::drivers::ble::mesh::bearer::{Bearer, Handler};
use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
pub use crate::drivers::ble::mesh::driver::node::MeshNodeMessage;
use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use core::cell::RefCell;
use core::future::Future;
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::{self, Channel, Receiver as ChannelReceiver};
use futures::future::select;
use futures::pin_mut;
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

const PDU_SIZE: usize = 384;

pub type NodeMutex = ThreadModeRawMutex;

pub struct MeshNode<'a, E, B, S, R>
where
    E: ElementsHandler<'a> + 'a,
    B: Bearer + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    channel: Channel<NodeMutex, Vec<u8, PDU_SIZE>, 6>,
    elements: Option<E>,
    force_reset: bool,
    capabilities: Option<Capabilities>,
    transport: B,
    storage: Option<S>,
    rng: Option<R>,
    node: Option<Node<'a, E, BearerTransmitter<'a, B>, BearerReceiver<'a>, S, R>>,
}

impl<'a, E, B, S, R> MeshNode<'a, E, B, S, R>
where
    E: ElementsHandler<'a>,
    B: Bearer,
    S: Storage,
    R: RngCore + CryptoRng,
{
    pub fn new(elements: E, capabilities: Capabilities, transport: B, storage: S, rng: R) -> Self {
        Self {
            channel: Channel::new(),
            elements: Some(elements),
            force_reset: false,
            capabilities: Some(capabilities),
            transport,
            storage: Some(storage),
            rng: Some(rng),
            node: None,
        }
    }

    pub fn force_reset(self) -> Self {
        Self {
            force_reset: true,
            ..self
        }
    }

    pub async fn run<const N: usize>(
        &'a mut self,
        control: ChannelReceiver<'_, NodeMutex, MeshNodeMessage, N>,
    ) {
        let sender = self.channel.sender();
        let receiver = self.channel.receiver();
        let tx = BearerTransmitter {
            transport: &self.transport,
        };
        let rx = BearerReceiver::new(receiver);
        let handler = BearerHandler::new(&self.transport, sender);

        let configuration_manager = ConfigurationManager::new(
            self.storage.take().unwrap(),
            self.elements.as_ref().unwrap().composition().clone(),
            self.force_reset,
        );

        self.node.replace(Node::new(
            self.elements.take().unwrap(),
            self.capabilities.take().unwrap(),
            tx,
            rx,
            configuration_manager,
            self.rng.take().unwrap(),
        ));

        let node_fut = self.node.as_mut().unwrap().run(control);
        let handler_fut = handler.start();
        pin_mut!(node_fut);
        pin_mut!(handler_fut);

        select(node_fut, handler_fut).await;
    }
}

struct BearerReceiver<'c> {
    receiver: RefCell<channel::Receiver<'c, NodeMutex, Vec<u8, PDU_SIZE>, 6>>,
}

impl<'c> BearerReceiver<'c> {
    fn new(receiver: channel::Receiver<'c, NodeMutex, Vec<u8, PDU_SIZE>, 6>) -> Self {
        Self {
            receiver: RefCell::new(receiver),
        }
    }
}

impl<'c> Receiver for BearerReceiver<'c> {
    type ReceiveFuture<'m> = impl Future<Output = Result<Vec<u8, PDU_SIZE>, DeviceError>>
    where
        Self: 'm;

    fn receive_bytes<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move { Ok(self.receiver.borrow_mut().recv().await) }
    }
}

struct BearerHandler<'t, 'c, B>
where
    B: Bearer + 't,
{
    transport: &'t B,
    sender: channel::Sender<'c, NodeMutex, Vec<u8, PDU_SIZE>, 6>,
}

impl<'t, 'c, B> BearerHandler<'t, 'c, B>
where
    B: Bearer + 't,
{
    fn new(transport: &'t B, sender: channel::Sender<'c, NodeMutex, Vec<u8, PDU_SIZE>, 6>) -> Self {
        Self { transport, sender }
    }

    async fn start(&self) -> Result<(), DeviceError> {
        self.transport.start_receive(self).await?;
        Ok(())
    }
}

impl<'t, 'c, B> Handler for BearerHandler<'t, 'c, B>
where
    B: Bearer + 't,
{
    fn handle(&self, message: Vec<u8, PDU_SIZE>) {
        // BLE loses messages anyhow, so if this fails, just ignore.
        self.sender.try_send(message).ok();
    }
}

struct BearerTransmitter<'t, B>
where
    B: Bearer + 't,
{
    transport: &'t B,
}

impl<'t, B> Transmitter for BearerTransmitter<'t, B>
where
    B: Bearer + 't,
{
    type TransmitFuture<'m> = impl Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn transmit_bytes<'m>(&'m self, bytes: &'m [u8]) -> Self::TransmitFuture<'m> {
        async move {
            self.transport.transmit(bytes).await?;
            Ok(())
        }
    }
}
