pub mod bearer;

use crate::drivers::ble::mesh::composition::ElementsHandler;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
pub use crate::drivers::ble::mesh::driver::node::MeshNodeMessage;
use crate::drivers::ble::mesh::driver::node::Node;
use crate::drivers::ble::mesh::interface::NetworkInterfaces;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use embassy_util::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_util::channel::mpmc::{Channel, DynamicReceiver as ChannelReceiver};
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

const PDU_SIZE: usize = 384;

pub type NodeMutex = ThreadModeRawMutex;

pub struct MeshNode<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    channel: Channel<NodeMutex, Vec<u8, PDU_SIZE>, 6>,
    elements: Option<E>,
    force_reset: bool,
    capabilities: Option<Capabilities>,
    network: Option<N>,
    storage: Option<S>,
    rng: Option<R>,
    node: Option<Node<'a, E, N, S, R>>,
}

impl<'a, E, N, S, R> MeshNode<'a, E, N, S, R>
where
    E: ElementsHandler<'a>,
    N: NetworkInterfaces,
    S: Storage,
    R: RngCore + CryptoRng,
{
    pub fn new(elements: E, capabilities: Capabilities, network: N, storage: S, rng: R) -> Self {
        Self {
            channel: Channel::new(),
            elements: Some(elements),
            force_reset: false,
            capabilities: Some(capabilities),
            network: Some(network),
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

    pub async fn run(&'a mut self, control: ChannelReceiver<'_, MeshNodeMessage>) {
        let configuration_manager = ConfigurationManager::new(
            self.storage.take().unwrap(),
            self.elements.as_ref().unwrap().composition().clone(),
            self.force_reset,
        );

        self.node.replace(Node::new(
            self.elements.take().unwrap(),
            self.capabilities.take().unwrap(),
            self.network.take().unwrap(),
            configuration_manager,
            self.rng.take().unwrap(),
        ));

        self.node.as_mut().unwrap().run(control).await.ok();
    }
}
