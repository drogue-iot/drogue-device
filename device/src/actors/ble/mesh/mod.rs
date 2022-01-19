pub mod transport;

use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::transport::{Handler, Transport};
use crate::drivers::ble::mesh::vault::Vault;
use crate::{Actor, Address, Inbox};
use core::cell::RefCell;
use core::future::Future;
use embassy::blocking_mutex::kind::CriticalSection;
use embassy::channel::mpsc::{self, Channel};
use futures::{join, pin_mut};
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

pub struct MeshNode<T, V, R>
where
    T: Transport + 'static,
    V: Vault + 'static,
    R: RngCore + CryptoRng + 'static,
{
    capabilities: Option<Capabilities>,
    transport: T,
    vault: Option<V>,
    rng: Option<R>,
}

impl<T, V, R> MeshNode<T, V, R>
where
    T: Transport + 'static,
    V: Vault + 'static,
    R: RngCore + CryptoRng + 'static,
{
    pub fn new(capabilities: Capabilities, transport: T, vault: V, rng: R) -> Self {
        Self {
            capabilities: Some(capabilities),
            transport,
            vault: Some(vault),
            rng: Some(rng),
        }
    }
}

struct TransportReceiver<'c> {
    receiver: RefCell<mpsc::Receiver<'c, CriticalSection, Vec<u8, 384>, 6>>,
}

impl<'c> TransportReceiver<'c> {
    fn new(receiver: mpsc::Receiver<'c, CriticalSection, Vec<u8, 384>, 6>) -> Self {
        Self {
            receiver: RefCell::new(receiver),
        }
    }
}

impl<'c> Receiver for TransportReceiver<'c> {
    type ReceiveFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Vec<u8, 384>, DeviceError>>;

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

struct TransportHandler<'t, 'c, T>
where
    T: Transport + 't,
{
    transport: &'t T,
    sender: mpsc::Sender<'c, CriticalSection, Vec<u8, 384>, 6>,
}

impl<'t, 'c, T> TransportHandler<'t, 'c, T>
where
    T: Transport + 't,
{
    fn new(transport: &'t T, sender: mpsc::Sender<'c, CriticalSection, Vec<u8, 384>, 6>) -> Self {
        Self { transport, sender }
    }

    async fn start(&self) {
        self.transport.start_receive(self).await
    }
}

impl<'t, 'c, T> Handler for TransportHandler<'t, 'c, T>
where
    T: Transport + 't,
{
    fn handle(&self, message: Vec<u8, 384>) {
        // BLE loses messages anyhow, so if this fails, just ignore.
        self.sender.try_send(message).ok();
    }
}

struct TransportTransmitter<'t, T>
where
    T: Transport + 't,
{
    transport: &'t T,
}

impl<'t, T> Transmitter for TransportTransmitter<'t, T>
where
    T: Transport + 't,
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

impl<T, V, R> Actor for MeshNode<T, V, R>
where
    T: Transport + 'static,
    V: Vault + 'static,
    R: RngCore + CryptoRng + 'static,
{
    type Message<'m> = Vec<u8, 384>;
    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let tx = TransportTransmitter {
                transport: &self.transport,
            };

            let mut channel = Channel::new();
            let (sender, receiver) = mpsc::split(&mut channel);

            let rx = TransportReceiver::new(receiver);
            let handler = TransportHandler::new(&self.transport, sender);

            let mut node = Node::new(
                self.capabilities.take().unwrap(),
                tx,
                rx,
                self.vault.take().unwrap(),
                self.rng.take().unwrap(),
            );

            let node_fut = node.run();
            let handler_fut = handler.start();

            pin_mut!(node_fut);
            pin_mut!(handler_fut);

            join!(node_fut, handler_fut);
        }
    }
}
