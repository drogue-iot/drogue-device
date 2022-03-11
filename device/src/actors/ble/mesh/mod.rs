pub mod bearer;

use crate::drivers::ble::mesh::bearer::{Bearer, Handler};
use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
pub use crate::drivers::ble::mesh::driver::node::MeshNodeMessage;
use crate::drivers::ble::mesh::driver::node::{
    ActivitySignal, NoOpActivitySignal, Node, Receiver, Transmitter,
};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use crate::{Actor, Address, Inbox};
use core::cell::RefCell;
use core::future::Future;
use embassy::blocking_mutex::kind::ThreadMode;
use embassy::channel::mpsc::{self, Channel};
use embassy::channel::signal::Signal;
use futures::future::{select, Either};
use futures::pin_mut;
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

const PDU_SIZE: usize = 384;

pub struct MeshNode<E, B, S, R, A>
where
    E: ElementsHandler,
    B: Bearer,
    S: Storage,
    R: RngCore + CryptoRng,
    A: ActivitySignal,
{
    channel: Channel<ThreadMode, Vec<u8, PDU_SIZE>, 6>,
    elements: Option<E>,
    force_reset: bool,
    capabilities: Option<Capabilities>,
    transport: B,
    storage: Option<S>,
    rng: Option<R>,
    activity: Option<A>,
}

impl<E, B, S, R, A> MeshNode<E, B, S, R, A>
where
    E: ElementsHandler,
    B: Bearer,
    S: Storage,
    R: RngCore + CryptoRng,
    A: ActivitySignal,
{
    pub fn new(
        elements: E,
        capabilities: Capabilities,
        transport: B,
        storage: S,
        rng: R,
        activity: A,
    ) -> Self {
        Self {
            channel: Channel::new(),
            elements: Some(elements),
            force_reset: false,
            capabilities: Some(capabilities),
            transport,
            storage: Some(storage),
            rng: Some(rng),
            activity: Some(activity),
        }
    }

    pub fn force_reset(self) -> Self {
        Self {
            force_reset: true,
            ..self
        }
    }
}

struct BearerReceiver<'c, 'a, A>
where
    A: ActivitySignal + 'a,
{
    receiver: RefCell<mpsc::Receiver<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>>,
    activity: &'a A,
}

impl<'c, 'a, A> BearerReceiver<'c, 'a, A>
where
    A: ActivitySignal + 'a,
{
    fn new(
        receiver: mpsc::Receiver<'c, ThreadMode, Vec<u8, PDU_SIZE>, 6>,
        activity: &'a A,
    ) -> Self {
        Self {
            receiver: RefCell::new(receiver),
            activity,
        }
    }
}

impl<'c, 'a, A> Receiver for BearerReceiver<'c, 'a, A>
where
    A: ActivitySignal + 'a,
{
    type ReceiveFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Vec<u8, PDU_SIZE>, DeviceError>>;

    fn receive_bytes<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move {
            loop {
                if let Some(bytes) = self.receiver.borrow_mut().recv().await {
                    self.activity.receive_start();
                    self.activity.receive_stop();
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

struct BearerTransmitter<'t, 'a, B, A>
where
    B: Bearer + 't,
    A: ActivitySignal + 'a,
{
    transport: &'t B,
    activity: &'a A,
}

impl<'t, 'a, B, A> Transmitter for BearerTransmitter<'t, 'a, B, A>
where
    B: Bearer + 't,
    A: ActivitySignal + 'a,
{
    type TransmitFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn transmit_bytes<'m>(&'m self, bytes: &'m [u8]) -> Self::TransmitFuture<'m> {
        async move {
            self.activity.transmit_start();
            self.transport.transmit(bytes).await?;
            self.activity.transmit_stop();
            Ok(())
        }
    }
}

impl<E, B, S, R, A> Actor for MeshNode<E, B, S, R, A>
where
    E: ElementsHandler + 'static,
    B: Bearer + 'static,
    S: Storage + 'static,
    R: RngCore + CryptoRng + 'static,
    A: ActivitySignal + 'static,
{
    type Message<'m> = MeshNodeMessage;
    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let (sender, receiver) = mpsc::split(&mut self.channel);

            let activity = self.activity.take().unwrap();

            let tx = BearerTransmitter {
                transport: &self.transport,
                activity: &activity,
            };

            let rx = BearerReceiver::new(receiver, &activity);
            let handler = BearerHandler::new(&self.transport, sender);

            let configuration_manager = ConfigurationManager::new(
                self.storage.take().unwrap(),
                self.elements.as_ref().unwrap().composition().clone(),
                self.force_reset,
            );

            let control_signal = Signal::new();

            let mut node = Node::new(
                self.elements.take().unwrap(),
                self.capabilities.take().unwrap(),
                tx,
                rx,
                configuration_manager,
                self.rng.take().unwrap(),
            );

            let node_fut = node.run(&control_signal);
            let handler_fut = handler.start();
            pin_mut!(node_fut);
            pin_mut!(handler_fut);

            let mut runtime_fut = select(node_fut, handler_fut);

            loop {
                let inbox_fut = inbox.next();
                pin_mut!(inbox_fut);

                let result = select(inbox_fut, runtime_fut).await;

                match result {
                    Either::Left((None, not_selected)) => {
                        runtime_fut = not_selected;
                    }
                    Either::Left((Some(mut message), not_selected)) => {
                        match &mut message.message() {
                            MeshNodeMessage::ForceReset => {
                                control_signal.signal(MeshNodeMessage::ForceReset);
                            }
                            _ => {
                                // todo: handle others.
                            }
                        }
                        runtime_fut = not_selected;
                    }
                    Either::Right((_, _)) => {
                        break;
                    }
                }
            }

            #[cfg(feature = "defmt")]
            info!("shutting down");
        }
    }
}
