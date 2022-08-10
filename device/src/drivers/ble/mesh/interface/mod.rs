pub mod advertising;
pub mod gatt;

use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use core::future::Future;
use embassy_util::{select, Either};
use futures::future::join;

use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::node::{NetworkId, State};
use crate::drivers::ble::mesh::interface::advertising::AdvertisingBearerNetworkInterface;
use crate::drivers::ble::mesh::interface::gatt::GattBearerNetworkInterface;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Debug)]
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

#[derive(Copy, Clone)]
pub enum Beacon {
    Unprovisioned,
    Provisioned(NetworkId),
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
    fn set_state(&self, state: State);

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
#[derive(Copy, Clone, Debug)]
pub enum BearerError {
    InvalidLink,
    InvalidTransaction,
    TransmissionFailure,
    InsufficientResources,
    ParseError(ParseError),
    Unspecified,
}

impl From<()> for BearerError {
    fn from(_: ()) -> Self {
        Self::ParseError(ParseError::InsufficientBuffer)
    }
}

impl From<ParseError> for BearerError {
    fn from(e: ParseError) -> Self {
        Self::ParseError(e)
    }
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
    fn set_state(&self, state: State);
    fn set_network_id(&self, network_id: NetworkId);

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

pub trait GattBearer<const MTU: usize> {
    fn set_state(&self, state: State);
    fn set_network_id(&self, network_id: NetworkId);

    type RunFuture<'m>: Future<Output = Result<(), BearerError>> + 'm
    where
        Self: 'm;

    fn run<'m>(&'m self) -> Self::RunFuture<'m>;

    type ReceiveFuture<'m>: Future<Output = Result<Vec<u8, MTU>, BearerError>> + 'm
    where
        Self: 'm;

    /// Receive data from the bearer.
    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m>;

    type TransmitFuture<'m>: Future<Output = Result<(), BearerError>> + 'm
    where
        Self: 'm;

    /// Transmit data on the bearer.
    fn transmit<'m>(&'m self, pdu: &'m Vec<u8, MTU>) -> Self::TransmitFuture<'m>;

    type AdvertiseFuture<'m>: Future<Output = Result<(), BearerError>> + 'm
    where
        Self: 'm;

    /// Transmit data on the bearer.
    fn advertise<'m>(&'m self, adv_data: &'m Vec<u8, 64>) -> Self::AdvertiseFuture<'m>;
}

pub struct AdvertisingAndGattNetworkInterfaces<
    AB: AdvertisingBearer,
    GB: GattBearer<MTU>,
    const MTU: usize,
> {
    advertising_interface: AdvertisingBearerNetworkInterface<AB>,
    gatt_interface: GattBearerNetworkInterface<GB, MTU>,
}

impl<AB: AdvertisingBearer, GB: GattBearer<MTU>, const MTU: usize>
    AdvertisingAndGattNetworkInterfaces<AB, GB, MTU>
{
    pub fn new(advertising_bearer: AB, gatt_bearer: GB) -> Self {
        Self {
            advertising_interface: AdvertisingBearerNetworkInterface::new(advertising_bearer),
            gatt_interface: GattBearerNetworkInterface::new(gatt_bearer),
        }
    }
}

impl<AB: AdvertisingBearer, GB: GattBearer<MTU>, const MTU: usize> NetworkInterfaces
    for AdvertisingAndGattNetworkInterfaces<AB, GB, MTU>
{
    fn set_state(&self, state: State) {
        self.advertising_interface.set_state(state);
        self.gatt_interface.set_state(state);
    }

    fn set_uuid(&self, uuid: Uuid) {
        //self.interface.set_uuid(uuid);
        self.advertising_interface.set_uuid(uuid);
        self.gatt_interface.set_uuid(uuid);
    }

    type RunFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn run<'m>(&'m self) -> Self::RunFuture<'m> {
        self.gatt_interface.run()
    }

    type ReceiveFuture<'m> = impl Future<Output=Result<PDU, NetworkError>> + 'm
    where
    Self: 'm;

    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move {
            let adv_fut = self.advertising_interface.receive();
            let gatt_fut = self.gatt_interface.receive();
            let result = select(adv_fut, gatt_fut).await;

            match result {
                Either::First(result) => Ok(result?),
                Either::Second(result) => Ok(result?),
            }
        }
    }

    type TransmitFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn transmit<'m>(&'m self, pdu: &'m PDU) -> Self::TransmitFuture<'m> {
        //async move { Ok(self.advertising_interface.transmit(pdu).await?) }
        async move {
            let gatt_fut = self.gatt_interface.transmit(pdu);
            let adv_fut = self.advertising_interface.transmit(pdu);

            let _result = join(gatt_fut, adv_fut).await;
            Ok(())
        }
    }

    type RetransmitFuture<'m> = impl Future<Output = Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn retransmit<'m>(&'m self) -> Self::RetransmitFuture<'m> {
        async move { Ok(self.advertising_interface.retransmit().await?) }
    }

    type BeaconFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn beacon<'m>(&'m self, beacon: Beacon) -> Self::BeaconFuture<'m> {
        async move {
            self.advertising_interface.beacon(beacon).await?;
            self.gatt_interface.beacon(beacon).await?;
            Ok(())
        }
    }
}

pub struct AdvertisingOnlyNetworkInterfaces<B: AdvertisingBearer> {
    interface: AdvertisingBearerNetworkInterface<B>,
}

impl<B: AdvertisingBearer> AdvertisingOnlyNetworkInterfaces<B> {
    pub fn new(bearer: B) -> Self {
        Self {
            interface: AdvertisingBearerNetworkInterface::new(bearer),
        }
    }
}

impl<B: AdvertisingBearer> NetworkInterfaces for AdvertisingOnlyNetworkInterfaces<B> {
    fn set_state(&self, state: State) {
        self.interface.set_state(state);
    }

    fn set_uuid(&self, uuid: Uuid) {
        self.interface.set_uuid(uuid);
    }

    type RunFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn run<'m>(&'m self) -> Self::RunFuture<'m> {
        async move {
            /* nothing */
            Ok(())
        }
    }

    type ReceiveFuture<'m> = impl Future<Output=Result<PDU, NetworkError>> + 'm
    where
    Self: 'm;

    fn receive<'m>(&'m self) -> Self::ReceiveFuture<'m> {
        async move { Ok(self.interface.receive().await?) }
    }

    type TransmitFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn transmit<'m>(&'m self, pdu: &'m PDU) -> Self::TransmitFuture<'m> {
        async move { Ok(self.interface.transmit(pdu).await?) }
    }

    type RetransmitFuture<'m> = impl Future<Output = Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn retransmit<'m>(&'m self) -> Self::RetransmitFuture<'m> {
        async move { Ok(self.interface.retransmit().await?) }
    }

    type BeaconFuture<'m> = impl Future<Output=Result<(), NetworkError>> + 'm
    where
    Self: 'm;

    fn beacon<'m>(&'m self, beacon: Beacon) -> Self::BeaconFuture<'m> {
        async move { Ok(self.interface.beacon(beacon).await?) }
    }
}
