use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
use crate::drivers::ble::mesh::config::network::NetworkKeyHandle;
use crate::drivers::ble::mesh::driver::elements::{
    AppElementsContext, ElementContext, Elements, PrimaryElementContext,
};
use crate::drivers::ble::mesh::driver::node::deadline::Deadline;
use crate::drivers::ble::mesh::driver::node::outbound::{
    Outbound, OutboundEvent, OutboundPublishMessage,
};
use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::pipeline::Pipeline;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::{Beacon, NetworkInterfaces};
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use crate::drivers::ble::mesh::vault::StorageVault;
use core::cell::{Cell, RefCell};
use embassy_executor::time::{Duration, Ticker};
use embassy_util::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_util::channel::mpmc::DynamicReceiver as ChannelReceiver;
use embassy_util::{select, select4, Either, Either4};
use futures::future::join;
use futures::StreamExt;
use rand_core::{CryptoRng, RngCore};
//use crate::drivers::ble::mesh::model::foundation::configuration::ConfigurationMessage::Beacon;

pub(crate) mod context;
pub(crate) mod deadline;
pub(crate) mod outbound;

type NodeMutex = ThreadModeRawMutex;

#[derive(Copy, Clone)]
pub struct NetworkId(pub [u8; 8]);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

pub struct Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a>,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    //
    state: Cell<State>,
    //
    network: N,
    configuration_manager: ConfigurationManager<S>,
    rng: RefCell<R>,
    pipeline: RefCell<Pipeline>,
    pub(crate) deadline: RefCell<Deadline>,
    //
    pub(crate) elements: RefCell<Elements<'a, E>>,
    pub(crate) outbound: Outbound<'a>,
}

impl<'a, E, N, S, R> Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a>,
    N: NetworkInterfaces,
    S: Storage,
    R: RngCore + CryptoRng,
{
    pub fn new(
        app_elements: E,
        capabilities: Capabilities,
        network: N,
        configuration_manager: ConfigurationManager<S>,
        rng: R,
    ) -> Self {
        let me = Self {
            state: Cell::new(State::Unprovisioned),
            network,
            configuration_manager,
            rng: RefCell::new(rng),
            pipeline: RefCell::new(Pipeline::new(capabilities)),
            deadline: RefCell::new(Default::default()),
            //
            elements: RefCell::new(Elements::new(app_elements)),
            outbound: Default::default(),
        };
        info!("State: {:?}", core::mem::size_of_val(&me.state));
        info!("Network: {:?}", core::mem::size_of_val(&me.network));
        info!(
            "COnfig: {:?}",
            core::mem::size_of_val(&me.configuration_manager)
        );
        info!("Rng: {:?}", core::mem::size_of_val(&me.rng));
        info!("Pipeline: {:?}", core::mem::size_of_val(&me.pipeline));
        info!("Deadline: {:?}", core::mem::size_of_val(&me.deadline));
        info!("Elements: {:?}", core::mem::size_of_val(&me.elements));
        info!("Outbound: {:?}", core::mem::size_of_val(&me.outbound));
        me
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
        debug!("State: unprovisioned");

        self.transmit_unprovisioned_beacon().await?;

        let receive_fut = self.network.receive();

        let mut ticker = Ticker::every(Duration::from_secs(3));
        let ticker_fut = ticker.next();

        //pin_mut!(receive_fut);
        //pin_mut!(ticker_fut);

        let result = select(receive_fut, ticker_fut).await;

        match result {
            Either::First(Ok(msg)) => self.pipeline.borrow_mut().process_inbound(self, msg).await,
            Either::Second(_) => {
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
        Ok(self.network.beacon(Beacon::Unprovisioned).await?)
    }

    async fn loop_provisioning(&self) -> Result<Option<State>, DeviceError> {
        debug!("State: provisioning");
        let receive_fut = self.network.receive();
        let mut ticker = Ticker::every(Duration::from_secs(1));
        let ticker_fut = ticker.next();

        let result = select(receive_fut, ticker_fut).await;

        let next_state = match result {
            Either::First(Ok(inbound)) => {
                let next_state = self
                    .pipeline
                    .borrow_mut()
                    .process_inbound(self, inbound)
                    .await;
                self.network.retransmit().await?;
                next_state
            }
            Either::Second(_) => {
                self.network.retransmit().await.ok();
                Ok(None)
            }
            _ => {
                // TODO handle this
                Ok(None)
            }
        };

        if let Ok(None) = next_state {
            if let Some(_) = self.configuration().network() {
                return Ok(Some(State::Provisioned));
            }
        }

        next_state
    }

    async fn transmit_provisioned_beacon(&self) -> Result<(), DeviceError> {
        if let Some(network) = self.configuration_manager.configuration().network() {
            if let Ok(network_id) = network.network_id() {
                debug!("pre-beacon");
                self.network.beacon(Beacon::Provisioned(network_id)).await?;
                debug!("post-beacon");
            }
        }
        Ok(())
    }

    async fn loop_provisioned(&self) -> Result<Option<State>, DeviceError> {
        debug!("State: provisioned");
        self.transmit_provisioned_beacon().await.ok();

        let mut deadline = self.deadline.borrow_mut();
        let deadline_fut = deadline.next();
        let receive_fut = self.network.receive();
        let outbound_fut = self.outbound.next();

        let mut ticker = Ticker::every(Duration::from_millis(100));
        let ticker_fut = ticker.next();

        let result = select4(receive_fut, outbound_fut, deadline_fut, ticker_fut).await;

        drop(deadline);

        match result {
            Either4::First(Ok(inbound)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, inbound)
                    .await
            }
            Either4::Second(outbound) => match outbound {
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
            Either4::Third(expiration) => {
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
                self.network.set_state(next_state);
            };
        }
        Ok(())
    }

    fn connect_elements(&'a self) {
        debug!("Connecting elements");
        let ctx: AppElementsContext<'a> = AppElementsContext {
            access_sender: self.outbound.access.sender(),
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
            error!("Error loading configuration {:?}", e);
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
        } else {
            if let Some(uuid) = self.configuration_manager.configuration().uuid() {
                self.network.set_uuid(*uuid);
            }
        }

        self.network.set_state(self.state.get());
        self.pipeline.borrow_mut().state(self.state.get());

        let network_fut = self.run_network();
        let node_fut = self.run_node(control);

        join(network_fut, node_fut).await;

        Ok(())
    }

    async fn run_network(&self) {
        self.network.run().await.ok();
    }

    async fn run_node(&'a self, control: ChannelReceiver<'_, MeshNodeMessage>) {
        loop {
            let loop_fut = self.do_loop();
            let signal_fut = control.recv();

            //pin_mut!(loop_fut);
            //pin_mut!(signal_fut);

            //let result = select(loop_fut, signal_fut).await;
            let result = select(loop_fut, signal_fut).await;

            match result {
                Either::First(_) => {}
                Either::Second(control_message) => match control_message {
                    MeshNodeMessage::ForceReset => {
                        self.configuration_manager.node_reset().await;
                    }
                    MeshNodeMessage::Shutdown => {}
                },
            }
        }
    }
}
