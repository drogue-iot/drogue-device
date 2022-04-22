use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::config::publications::Publication;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::network_transmit::NetworkTransmitConfig;
use crate::drivers::ble::mesh::model::foundation::configuration::relay::RelayConfig;
use crate::drivers::ble::mesh::pdu::bearer::advertising;
use crate::drivers::ble::mesh::pdu::bearer::advertising::AdvertisingPDU;
use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use crate::drivers::ble::mesh::pdu::{network, ParseError};
use crate::drivers::ble::mesh::{MESH_MESSAGE, PB_ADV};
use core::future::Future;
use embassy::time::Duration;

#[derive(Copy, Clone)]
pub struct NetworkRetransmitDetails {
    pub(crate) count: u8,
    pub(crate) interval: Duration,
}

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
            interval: Duration::from_millis( ( publication.publish_retransmit_interval_steps as u64 + 1) * 50),
        }
    }
}

pub trait MeshContext {
    fn uuid(&self) -> Uuid;

    fn network_retransmit(&self) -> NetworkRetransmitDetails;

    type TransmitAdvertisingFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn transmit_advertising_pdu<'m>(
        &'m self,
        pdu: AdvertisingPDU,
    ) -> Self::TransmitAdvertisingFuture<'m>;

    type TransmitMeshFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    /// Actually transmit the PDU onto the wire.
    fn transmit_mesh_pdu<'m>(
        &'m self,
        pdu: &'m ObfuscatedAndEncryptedNetworkPDU,
    ) -> Self::TransmitMeshFuture<'m>;

    fn primary_unicast_address(&self) -> Result<UnicastAddress, DeviceError>;

    fn is_local_unicast(&self, addr: &Address) -> bool;
}

pub struct Mesh {}

pub enum MeshData {
    Provisioning(advertising::AdvertisingPDU),
    Network(network::ObfuscatedAndEncryptedNetworkPDU),
}

impl Default for Mesh {
    fn default() -> Self {
        Self {}
    }
}

#[allow(unused_variables)]
impl Mesh {
    pub fn process_inbound<C: MeshContext>(
        &mut self,
        ctx: &C,
        data: &[u8],
    ) -> Result<Option<MeshData>, DeviceError> {
        if data.len() >= 2 {
            if data[1] == PB_ADV {
                Ok(Some(MeshData::Provisioning(
                    advertising::AdvertisingPDU::parse(data)
                        .map_err(|_| DeviceError::InvalidPacket)?,
                )))
            } else if data[1] == MESH_MESSAGE {
                let len = data[0] as usize;
                if data.len() >= len + 1 {
                    Ok(Some(MeshData::Network(
                        network::ObfuscatedAndEncryptedNetworkPDU::parse(&data[2..2 + len - 1])
                            .map_err(|_| DeviceError::InvalidPacket)?,
                    )))
                } else {
                    Err(DeviceError::ParseError(ParseError::InvalidLength))
                }
            } else {
                Err(DeviceError::InvalidPacket)
            }
        } else {
            Ok(None)
        }
    }
}
