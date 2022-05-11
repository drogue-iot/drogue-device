pub mod pb_adv;

use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use core::future::Future;

use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone)]
pub enum NetworkError {
    InvalidLink,
    InvalidTransaction,
    Unspecified,
    Bearer(BearerError),
}

impl From<BearerError> for NetworkError {
    fn from(err: BearerError) -> Self {
        NetworkError::Bearer(err)
    }
}

pub enum Beacon {
    Unprovisioned,
    Secure,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PDU {
    Provisioning(ProvisioningPDU),
    Network(ObfuscatedAndEncryptedNetworkPDU),
}

/// A possibly plurality of network interfaces covering one or more bearers.
///
/// Implementations should include whatever input and output buffering that
/// makes sense for their underlying bearers.
pub trait NetworkInterfaces {
    fn set_uuid(&self, uuid: Uuid);

    type RunFuture<'m>: Future<Output = Result<(), NetworkError>> + 'm
    where
        Self: 'm;

    /// Run the network interfaces, stopping when the future is dropped.
    fn run<'m>(&'m self) -> Self::RunFuture<'m>;

    type ReceiveFuture<'m>: Future<Output = Result<PDU, NetworkError>> + 'm
    where
        Self: 'm;

    /// Receive data from any of the network interfaces.
    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m>;

    type TransmitFuture<'m>: Future<Output = Result<(), NetworkError>> + 'm
    where
        Self: 'm;

    /// Transmit data on all of the network interfaces.
    fn transmit<'m>(&'m self, pdu: &'m PDU) -> Self::TransmitFuture<'m>;

    type RetransmitFuture<'m>: Future<Output = Result<(), NetworkError>> + 'm
    where
        Self: 'm;

    /// Retransmit any necessary network-level packets held by the interfaces.
    fn retransmit<'m>(&'m self) -> Self::RetransmitFuture<'m>;

    type BeaconFuture<'m>: Future<Output = Result<(), NetworkError>> + 'm
    where
        Self: 'm;

    /// Perform beaconing on all of the network interfaces.
    fn beacon<'m>(&'m self, beacon: Beacon) -> Self::BeaconFuture<'m>;
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone)]
pub enum BearerError {
    InvalidLink,
    InvalidTransaction,
    TransmissionFailure,
    InsufficientResources,
    Unspecified,
}

impl From<InsufficientBuffer> for BearerError {
    fn from(_: InsufficientBuffer) -> Self {
        BearerError::InsufficientResources
    }
}

// For heapless Vec::push
impl From<u8> for BearerError {
    fn from(_: u8) -> Self {
        BearerError::InsufficientResources
    }
}

pub const PB_ADV_MTU: usize = 64;

pub trait AdvertisingBearer {
    type ReceiveFuture<'m>: Future<Output = Result<Vec<u8, PB_ADV_MTU>, BearerError>> + 'm
    where
        Self: 'm;

    /// Receive data from the bearer.
    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m>;

    type TransmitFuture<'m>: Future<Output = Result<(), BearerError>> + 'm
    where
        Self: 'm;

    /// Transmit data on the bearer.
    fn transmit<'m>(&'m self, pdu: &'m Vec<u8, PB_ADV_MTU>) -> Self::TransmitFuture<'m>;
}
