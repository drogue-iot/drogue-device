use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
use crate::drivers::ble::mesh::config::network::NetworkKeyHandle;
use crate::drivers::ble::mesh::driver::elements::{AppElementsContext, ElementContext, Elements};
use crate::drivers::ble::mesh::driver::node::deadline::{Deadline, Expiration};
use crate::drivers::ble::mesh::driver::node::outbound::{
    Outbound, OutboundEvent, OutboundPublishMessage,
};
use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::pipeline::Pipeline;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use crate::drivers::ble::mesh::vault::{StorageVault, Vault};
use crate::drivers::ble::mesh::MESH_BEACON;
use core::cell::{Cell, RefCell};
use core::future::Future;
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::DynamicReceiver as ChannelReceiver;
use embassy::time::{Duration, Ticker};
use embassy::util::{select3, Either3};
use futures::future::{select, Either};
use futures::{pin_mut, StreamExt};
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

pub(crate) mod context;
pub(crate) mod deadline;
pub(crate) mod outbound;

type NodeMutex = ThreadModeRawMutex;

pub trait Transmitter {
    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;
    fn transmit_bytes<'m>(&'m self, bytes: &'m [u8]) -> Self::TransmitFuture<'m>;
}

pub trait Receiver {
    type ReceiveFuture<'m>: Future<Output = Result<Vec<u8, 384>, DeviceError>>
    where
        Self: 'm;
    fn receive_bytes<'m>(&'m self) -> Self::ReceiveFuture<'m>;
}

#[derive(Copy, Clone, PartialEq)]
pub enum State {
    Unprovisioned,
    Provisioning,
    Provisioned,
}

pub enum MeshNodeMessage {
    ForceReset,
    Shutdown,
}

pub struct Node<'a, E, TX, RX, S, R>
where
    E: ElementsHandler<'a>,
    TX: Transmitter + 'a,
    RX: Receiver + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    //
    state: Cell<State>,
    //
    transmitter: TX,
    receiver: RX,
    configuration_manager: ConfigurationManager<S>,
    rng: RefCell<R>,
    pipeline: RefCell<Pipeline>,
    pub(crate) deadline: RefCell<Deadline>,
    //
    pub(crate) elements: RefCell<Elements<'a, E>>,
    pub(crate) outbound: Outbound<'a>,
}

impl<'a, E, TX, RX, S, R> Node<'a, E, TX, RX, S, R>
where
    E: ElementsHandler<'a>,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    pub fn new(
        app_elements: E,
        capabilities: Capabilities,
        transmitter: TX,
        receiver: RX,
        configuration_manager: ConfigurationManager<S>,
        rng: R,
    ) -> Self {
        Self {
            state: Cell::new(State::Unprovisioned),
            transmitter,
            receiver,
            configuration_manager,
            rng: RefCell::new(rng),
            pipeline: RefCell::new(Pipeline::new(capabilities)),
            deadline: RefCell::new(Default::default()),
            //
            elements: RefCell::new(Elements::new(app_elements)),
            outbound: Default::default(),
        }
    }

    pub(crate) fn vault(&self) -> StorageVault<S> {
        StorageVault::new(&self.configuration_manager)
    }

    async fn publish(&self, publish: OutboundPublishMessage) -> Result<(), DeviceError> {
        let network = self.configuration_manager.configuration().network().clone();
        if let Some(network) = network {
            if let Some((network, publication)) =
                network.find_publication(&publish.element_address, &publish.model_identifier)
            {
                if let Some(app_key_details) =
                    network.find_app_key_by_index(&publication.app_key_index)
                {
                    let model_key = publish.model_key();
                    let message = AccessMessage {
                        ttl: publication.publish_ttl,
                        network_key: NetworkKeyHandle::from(network),
                        // todo: ivi isn't always zero.
                        ivi: 0,
                        nid: network.nid,
                        akf: true,
                        aid: app_key_details.aid,
                        src: publish.element_address,
                        dst: publication.publish_address,
                        payload: publish.payload,
                    };
                    self.pipeline
                        .borrow_mut()
                        .process_outbound(
                            self,
                            &message,
                            Some((model_key, publication.into())),
                            self.network_retransmit(),
                        )
                        .await?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn loop_unprovisioned(&self) -> Result<Option<State>, DeviceError> {
        self.transmit_unprovisioned_beacon().await?;

        let receive_fut = self.receiver.receive_bytes();

        let mut ticker = Ticker::every(Duration::from_secs(3));
        let ticker_fut = ticker.next();

        pin_mut!(receive_fut);
        pin_mut!(ticker_fut);

        let result = select(receive_fut, ticker_fut).await;

        match result {
            Either::Left((Ok(msg), _)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, &*msg)
                    .await
            }
            Either::Right((_, _)) => {
                self.transmit_unprovisioned_beacon().await?;
                Ok(None)
            }
            _ => {
                // TODO handle this
                Ok(None)
            }
        }
    }

    async fn transmit_unprovisioned_beacon(&self) -> Result<(), DeviceError> {
        let mut adv_data: Vec<u8, 31> = Vec::new();
        adv_data.extend_from_slice(&[20, MESH_BEACON, 0x00]).ok();
        adv_data.extend_from_slice(&self.vault().uuid().0).ok();
        adv_data.extend_from_slice(&[0xa0, 0x40]).ok();

        self.transmitter.transmit_bytes(&*adv_data).await
    }

    async fn loop_provisioning(&self) -> Result<Option<State>, DeviceError> {
        let receive_fut = self.receiver.receive_bytes();
        let mut ticker = Ticker::every(Duration::from_secs(1));
        let ticker_fut = ticker.next();

        pin_mut!(receive_fut);
        pin_mut!(ticker_fut);

        let result = select(receive_fut, ticker_fut).await;

        match result {
            Either::Left((Ok(inbound), _)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, &*inbound)
                    .await
            }
            Either::Right((_, _)) => {
                self.pipeline.borrow_mut().try_retransmit(self).await?;
                Ok(None)
            }
            _ => {
                // TODO handle this
                Ok(None)
            }
        }
    }

    async fn loop_provisioned(&self) -> Result<Option<State>, DeviceError> {
        let mut deadline = self.deadline.borrow_mut();
        let deadline_fut = deadline.next();
        let receive_fut = self.receiver.receive_bytes();
        let outbound_fut = self.outbound.next();

        let result = select3(receive_fut, outbound_fut, deadline_fut).await;

        match result {
            Either3::First(Ok(inbound)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, &*inbound)
                    .await
            }
            Either3::Second(outbound) => match outbound {
                OutboundEvent::Access(access) => {
                    self.pipeline
                        .borrow_mut()
                        .process_outbound(self, &access, None, self.network_retransmit())
                        .await?;
                    Ok(None)
                }
                OutboundEvent::Publish(publish) => {
                    self.publish(publish).await?;
                    Ok(None)
                }
            },
            Either3::Third(expiration) => {
                match expiration {
                    Expiration::Network => {}
                    Expiration::Publish => {}
                    Expiration::Ack => {}
                }
                // TODO chunk into the correct portion of the pipeline.
                self.pipeline.borrow_mut().try_retransmit(self).await?;
                self.pipeline
                    .borrow_mut()
                    .retransmit(self, expiration)
                    .await?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    async fn do_loop(&'a self) -> Result<(), DeviceError> {
        let current_state = self.state.get();

        if let Some(next_state) = match current_state {
            State::Unprovisioned => self.loop_unprovisioned().await,
            State::Provisioning => self.loop_provisioning().await,
            State::Provisioned => self.loop_provisioned().await,
        }? {
            if matches!(next_state, State::Provisioned) {
                if !matches!(current_state, State::Provisioned) {
                    // only connect during the first transition.
                    self.connect_elements()
                }
            }
            if next_state != current_state {
                self.state.set(next_state);
                self.pipeline.borrow_mut().state(next_state);
            };
        }
        Ok(())
    }

    fn connect_elements(&'a self) {
        let ctx: AppElementsContext<'a> = AppElementsContext {
            sender: self.outbound.publish.sender(),
            address: self.address().unwrap(),
        };
        self.elements.borrow_mut().connect(ctx);
    }

    pub async fn run(
        &'a self,
        control: ChannelReceiver<'_, MeshNodeMessage>,
    ) -> Result<(), DeviceError> {
        let mut rng = self.rng.borrow_mut();
        if let Err(e) = self.configuration_manager.initialize(&mut *rng).await {
            // try again as a force reset
            error!("Error loading configuration {}", e);
            warn!("Unable to load configuration; attempting reset.");
            self.configuration_manager.reset();
            self.configuration_manager.initialize(&mut *rng).await?
        }

        drop(rng);

        #[cfg(feature = "defmt")]
        self.configuration_manager.display_configuration();

        if self.configuration_manager.is_provisioned() {
            self.state.set(State::Provisioned);
            self.connect_elements();
        }

        self.pipeline.borrow_mut().state(self.state.get());

        loop {
            let loop_fut = self.do_loop();
            let signal_fut = control.recv();

            pin_mut!(loop_fut);
            pin_mut!(signal_fut);

            let result = select(loop_fut, signal_fut).await;

            match &result {
                Either::Left((_, _)) => {
                    // normal operation
                }
                Either::Right((control_message, _)) => match control_message {
                    MeshNodeMessage::ForceReset => {
                        self.configuration_manager.node_reset().await;
                    }
                    MeshNodeMessage::Shutdown => {}
                },
            }
        }
    }
}
