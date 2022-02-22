pub mod bearer;

use crate::drivers::ble::mesh::bearer::{Bearer, Handler};
use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::configuration_manager::ConfigurationManager;
use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use crate::{Actor, Address, Inbox};
use core::cell::RefCell;
use core::future::Future;
use embassy::blocking_mutex::kind::ThreadMode;
use embassy::channel::mpsc::{self, Channel};
use futures::{join, pin_mut};
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

const PDU_SIZE: usize = 384;

pub struct MeshNode<E, B, S, R>
where
    E: ElementsHandler,
    B: Bearer,
    S: Storage,
    R: RngCore + CryptoRng,
{
    channel: Channel<ThreadMode, Vec<u8, PDU_SIZE>, 6>,
    elements: Option<E>,
    force_reset: bool,
    capabilities: Option<Capabilities>,
    transport: B,
    storage: Option<S>,
    rng: Option<R>,
}

impl<E, B, S, R> MeshNode<E, B, S, R>
where
    E: ElementsHandler,
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
        }
    }

    pub fn force_reset(self) -> Self {
        Self {
            force_reset: true,
            ..self
        }
    }
}

struct BearerReceiver<'c> {
    receiver: RefCell<mpsc::Receiver<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>>,
}

impl<'c> BearerReceiver<'c> {
    fn new(receiver: mpsc::Receiver<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>) -> Self {
        Self {
            receiver: RefCell::new(receiver),
        }
    }
}

impl<'c> Receiver for BearerReceiver<'c> {
    type ReceiveFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Vec<u8, PDU_SIZE>, DeviceError>>;

    fn receive_bytes<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move {
            loop {
                if let Some(bytes) = self.receiver.borrow_mut().recv().await {
                    return Ok(bytes);
                }
            }
        }
    }
}

struct BearerHandler<'t, 'c, B>
where
    B: Bearer + 't,
{
    transport: &'t B,
    sender: mpsc::Sender<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>,
}

impl<'t, 'c, B> BearerHandler<'t, 'c, B>
where
    B: Bearer + 't,
{
    fn new(transport: &'t B, sender: mpsc::Sender<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>) -> Self {
        Self { transport, sender }
    }

    async fn start(&self) {
        self.transport.start_receive(self).await
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
    type TransmitFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn transmit_bytes<'m>(&'m self, bytes: &'m [u8]) -> Self::TransmitFuture<'m> {
        async move {
            self.transport.transmit(bytes).await;
            Ok(())
        }
    }
}

impl<E, B, S, R> Actor for MeshNode<E, B, S, R>
where
    E: ElementsHandler + 'static,
    B: Bearer + 'static,
    S: Storage + 'static,
    R: RngCore + CryptoRng + 'static,
{
    type Message<'m> = Vec<u8, PDU_SIZE>;
    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let (sender, receiver) = mpsc::split(&mut self.channel);

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

            let mut node = Node::new(
                self.elements.take().unwrap(),
                self.capabilities.take().unwrap(),
                tx,
                rx,
                configuration_manager,
                self.rng.take().unwrap(),
            );

            let node_fut = node.run();
            let handler_fut = handler.start();

            pin_mut!(node_fut);
            pin_mut!(handler_fut);

            let _ = join!(node_fut, handler_fut);

            info!("shutting down");
        }
    }
}
