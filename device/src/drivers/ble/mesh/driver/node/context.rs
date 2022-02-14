use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::composition::{Composition, ElementsHandler};
use crate::drivers::ble::mesh::configuration_manager::{
    KeyStorage, NetworkKeyStorage, PrimaryElementModels, PrimaryElementStorage,
};
use crate::drivers::ble::mesh::crypto::nonce::DeviceNonce;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::elements::{
    ElementContext, NetworkDetails, PrimaryElementContext,
};
use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::AccessContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::LowerContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::RelayContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::UpperContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::ProvisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::UnprovisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::PipelineContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyIndex, NetKeyIndex};
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::bearer::advertising::AdvertisingPDU;
use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use crate::drivers::ble::mesh::provisioning::ProvisioningData;
use crate::drivers::ble::mesh::storage::Storage;
use crate::drivers::ble::mesh::vault::Vault;
use crate::drivers::ble::mesh::{crypto, MESH_MESSAGE};
use aes::Aes128;
use cmac::crypto_mac::Output;
use cmac::Cmac;
use core::convert::TryInto;
use core::future::Future;
use heapless::Vec;
use p256::PublicKey;
use rand_core::{CryptoRng, RngCore};

// ------------------------------------------------------------------------
// Unprovisioned pipeline context
// ------------------------------------------------------------------------

impl<E, TX, RX, S, R> UnprovisionedContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    fn rng_fill(&self, dest: &mut [u8]) {
        self.rng.borrow_mut().fill_bytes(dest);
    }

    type SetPeerPublicKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_peer_public_key<'m>(&'m self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m> {
        async move { self.vault().set_peer_public_key(pk).await }
    }

    fn public_key(&self) -> Result<PublicKey, DeviceError> {
        self.vault().public_key()
    }

    type SetProvisioningDataFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_provisioning_data<'m>(
        &'m self,
        provisioning_salt: &'m [u8],
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m> {
        async move {
            self.vault()
                .set_provisioning_data(provisioning_salt, data)
                .await
        }
    }

    fn aes_cmac(&self, key: &[u8], input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.vault().aes_cmac(key, input)
    }

    fn s1(&self, input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::s1(input).map_err(|_| DeviceError::InvalidKeyLength)
    }

    fn prsk(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.vault().prsk(salt)
    }

    fn prsn(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.vault().prsn(salt)
    }

    fn prck(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.vault().prck(salt)
    }

    fn aes_ccm_decrypt(
        &self,
        key: &[u8],
        nonce: &[u8],
        data: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        crypto::aes_ccm_decrypt_detached(key, nonce, data, mic)
            .map_err(|_| DeviceError::CryptoError)
    }

    fn rng_u8(&self) -> u8 {
        (self.rng.borrow_mut().next_u32() & 0xFF) as u8
    }

    fn rng_u32(&self) -> u32 {
        self.rng.borrow_mut().next_u32()
    }
}

impl<E, TX, RX, S, R> MeshContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    fn uuid(&self) -> Uuid {
        self.vault().uuid()
    }

    type TransmitAdvertisingFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn transmit_advertising_pdu<'m>(
        &'m self,
        pdu: AdvertisingPDU,
    ) -> Self::TransmitAdvertisingFuture<'m> {
        async move {
            let mut bytes = Vec::<u8, 64>::new();
            pdu.emit(&mut bytes)
                .map_err(|_| DeviceError::InsufficientBuffer)?;
            self.transmitter.transmit_bytes(&*bytes).await
        }
    }

    type TransmitMeshFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn transmit_mesh_pdu<'m>(
        &'m self,
        pdu: &'m ObfuscatedAndEncryptedNetworkPDU,
    ) -> Self::TransmitMeshFuture<'m> {
        async move {
            let mut bytes = Vec::<u8, 64>::new();
            bytes
                .push(0x00)
                .map_err(|_| DeviceError::InsufficientBuffer)?; // length placeholder
            bytes
                .push(MESH_MESSAGE)
                .map_err(|_| DeviceError::InsufficientBuffer)?;
            pdu.emit(&mut bytes)
                .map_err(|_| DeviceError::InsufficientBuffer)?;
            bytes[0] = bytes.len() as u8 - 1;
            self.transmitter.transmit_bytes(&*bytes).await
        }
    }
}

// ------------------------------------------------------------------------
// Provisioned pipeline context
// ------------------------------------------------------------------------

impl<E, TX, RX, S, R> ProvisionedContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
}

impl<E, TX, RX, S, R> RelayContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
    fn is_local_unicast(&self, address: &Address) -> bool {
        self.vault().is_local_unicast(address)
    }
}

impl<E, TX, RX, S, R> AuthenticationContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
    fn iv_index(&self) -> Option<u32> {
        self.vault().iv_index()
    }

    fn network_keys(&self, nid: u8) -> Vec<NetworkKeyStorage, 10> {
        self.vault().network_keys(nid)
    }
}

impl<E, TX, RX, S, R> LowerContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
    fn decrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        self.vault().decrypt_device_key(nonce, bytes, mic)
    }

    fn encrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
    ) -> Result<(), DeviceError> {
        self.vault().encrypt_device_key(nonce, bytes, mic)
    }

    type NextSequenceFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<u32, DeviceError>> + 'm;

    fn next_sequence<'m>(&'m self) -> Self::NextSequenceFuture<'m> {
        async move { self.configuration_manager.next_sequence().await }
    }

    fn default_ttl(&self) -> u8 {
        PrimaryElementStorage::retrieve(&self.configuration_manager)
            .configuration
            .default_ttl
    }
}

impl<E, TX, RX, S, R> UpperContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
}

impl<E, TX, RX, S, R> AccessContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
    type DispatchFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>> + 'm;

    fn dispatch_access<'m>(&'m self, message: &'m AccessMessage) -> Self::DispatchFuture<'m> {
        async move { self.elements.dispatch(self, message).await }
    }
}

impl<E, TX, RX, S, R> PipelineContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
}

impl<E, TX, RX, S, R> ElementContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    R: CryptoRng + RngCore,
    RX: Receiver,
    S: Storage,
    TX: Transmitter,
{
    type TransmitFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>> + 'm;

    fn transmit<'m>(&'m self, message: AccessMessage) -> Self::TransmitFuture<'m> {
        async move {
            self.outbound.send(message).await;
            Ok(())
        }
    }

    fn address(&self) -> Option<UnicastAddress> {
        // todo element-specific addresses
        let keys = KeyStorage::retrieve(&self.configuration_manager);
        if let Some(network) = keys.network() {
            network.unicast_address.try_into().ok()
        } else {
            None
        }
    }
}

impl<E, TX, RX, S, R> PrimaryElementContext for Node<E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    fn retrieve(&self) -> PrimaryElementModels {
        PrimaryElementStorage::retrieve(&self.configuration_manager)
    }

    type StoreFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>> + 'm;

    fn store<'m>(&'m self, update: PrimaryElementModels) -> Self::StoreFuture<'m> {
        PrimaryElementStorage::store(&self.configuration_manager, update)
    }

    type NodeResetFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = ()> + 'm;

    fn node_reset<'m>(&'m self) -> Self::NodeResetFuture<'m> {
        async move { self.configuration_manager.node_reset().await }
    }

    fn composition(&self) -> Composition {
        self.configuration_manager.composition()
    }

    type NetworkDetails<'n>
    where
        Self: 'n,
        E: 'n,
        TX: 'n,
        RX: 'n,
        S: 'n,
        R: 'n,
    = NodeNetworkDetails<'n, E, TX, RX, S, R>;

    fn network_details(&self, net_key_index: NetKeyIndex) -> Option<Self::NetworkDetails<'_>> {
        Some(NodeNetworkDetails {
            node: self,
            net_key_index,
        })
    }

    fn network_details_by_app_key(
        &self,
        app_key_index: AppKeyIndex,
    ) -> Option<Self::NetworkDetails<'_>> {
        if let Some(net_key_index) = self
            .configuration_manager
            .net_key_index_by_app_key_index(app_key_index)
        {
            Some(NodeNetworkDetails {
                node: self,
                net_key_index,
            })
        } else {
            None
        }
    }
}

pub struct NodeNetworkDetails<'n, E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    node: &'n Node<E, TX, RX, S, R>,
    net_key_index: NetKeyIndex,
}

impl<'n, E, TX, RX, S, R> NetworkDetails for NodeNetworkDetails<'n, E, TX, RX, S, R>
where
    E: ElementsHandler,
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    type AddKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Status, DeviceError>> + 'm;

    fn add_app_key(&mut self, app_key_index: AppKeyIndex, key: [u8; 16]) -> Self::AddKeyFuture<'_> {
        self.node
            .configuration_manager
            .add_app_key(self.net_key_index, app_key_index, key)
    }

    fn app_key_indexes(&self) -> Result<Vec<AppKeyIndex, 10>, Status> {
        self.node
            .configuration_manager
            .app_key_indexes(self.net_key_index)
    }

    type ModelAppBindFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Status, DeviceError>> + 'm;

    fn model_app_bind<'m>(
        &'m self,
        element_address: UnicastAddress,
        model_identifier: ModelIdentifier,
        app_key_index: AppKeyIndex,
    ) -> Self::ModelAppBindFuture<'m> {
        self.node.configuration_manager.model_app_bind(
            self.net_key_index,
            element_address,
            model_identifier,
            app_key_index,
        )
    }

    type ModelAppUnbindFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Status, DeviceError>> + 'm;

    fn model_app_unbind<'m>(
        &'m self,
        element_address: UnicastAddress,
        model_identifier: ModelIdentifier,
        app_key_index: AppKeyIndex,
    ) -> Self::ModelAppUnbindFuture<'m> {
        self.node.configuration_manager.model_app_unbind(
            self.net_key_index,
            element_address,
            model_identifier,
            app_key_index,
        )
    }

    type ModelPublicationSetFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Status, DeviceError>> + 'm;

    fn model_publication_set<'m>(
        &'m self,
        element_address: UnicastAddress,
        publish_address: Address,
        app_key_index: AppKeyIndex,
        credential_flag: bool,
        publish_ttl: u8,
        publish_period: u8,
        publish_retransmit_count: u8,
        publish_retransmit_interval_steps: u8,
        model_identifier: ModelIdentifier,
    ) -> Self::ModelPublicationSetFuture<'m> {
        self.node.configuration_manager.model_publication_set(
            element_address,
            publish_address,
            app_key_index,
            credential_flag,
            publish_ttl,
            publish_period,
            publish_retransmit_count,
            publish_retransmit_interval_steps,
            model_identifier,
        )
    }
}
