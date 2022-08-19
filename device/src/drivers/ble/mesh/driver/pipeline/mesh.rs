use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::config::publications::Publication;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::PDU;
use crate::drivers::ble::mesh::model::foundation::configuration::network_transmit::NetworkTransmitConfig;
#[cfg(feature = "ble-mesh-relay")]
use crate::drivers::ble::mesh::model::foundation::configuration::relay::RelayConfig;
use core::future::Future;
use embassy_time::Duration;

#[derive(Copy, Clone)]
pub struct NetworkRetransmitDetails {
    pub(crate) count: u8,
    pub(crate) interval: Duration,
}

#[cfg(feature = "ble-mesh-relay")]
impl From<&RelayConfig> for NetworkRetransmitDetails {
    fn from(config: &RelayConfig) -> Self {
        NetworkRetransmitDetails {
            count: config.relay_retransmit_count,
            interval: Duration::from_millis(
                (config.relay_retransmit_interval_steps as u64 + 1) * 10,
            ),
        }
    }
}

impl From<&NetworkTransmitConfig> for NetworkRetransmitDetails {
    fn from(config: &NetworkTransmitConfig) -> Self {
        NetworkRetransmitDetails {
            count: config.network_retransmit_count,
            interval: Duration::from_millis(
                (config.network_retransmit_interval_steps as u64 + 1) * 10,
            ),
        }
    }
}

#[derive(Copy, Clone)]
pub struct PublishRetransmitDetails {
    pub(crate) count: u8,
    pub(crate) interval: Duration,
}

impl From<&Publication> for PublishRetransmitDetails {
    fn from(publication: &Publication) -> Self {
        PublishRetransmitDetails {
            count: publication.publish_retransmit_count,
            interval: Duration::from_millis(
                (publication.publish_retransmit_interval_steps as u64 + 1) * 50,
            ),
        }
    }
}

pub trait MeshContext {
    fn uuid(&self) -> Uuid;

    fn network_retransmit(&self) -> NetworkRetransmitDetails;

    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    /// Actually transmit the PDU onto the wire.
    fn transmit<'m>(&'m self, pdu: &'m PDU) -> Self::TransmitFuture<'m>;

    fn primary_unicast_address(&self) -> Result<UnicastAddress, DeviceError>;

    fn is_local_unicast(&self, addr: &Address) -> bool;
}
