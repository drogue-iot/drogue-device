use crate::drivers::ble::mesh::address::{Address, LabelUuid, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::composition::{Composition, ElementsHandler};
use crate::drivers::ble::mesh::config::network::NetworkDetails;
use crate::drivers::ble::mesh::config::Configuration;
use crate::drivers::ble::mesh::crypto;
use crate::drivers::ble::mesh::crypto::nonce::{ApplicationNonce, DeviceNonce};
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::elements::{ElementContext, PrimaryElementContext};
use crate::drivers::ble::mesh::driver::node::outbound::OutboundPublishMessage;
use crate::drivers::ble::mesh::driver::node::Node;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{MeshContext, NetworkRetransmitDetails};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::AccessContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::LowerContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
#[cfg(feature = "ble-mesh-relay")]
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::RelayContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::NetworkContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::UpperContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::ProvisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::UnprovisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::PipelineContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::{NetworkInterfaces, PDU};
#[cfg(feature = "ble-mesh-relay")]
use crate::drivers::ble::mesh::model::foundation::configuration::relay::Relay;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::provisioning::ProvisioningData;
use crate::drivers::ble::mesh::storage::Storage;
use crate::drivers::ble::mesh::vault::Vault;
use aes::Aes128;
use cmac::crypto_mac::Output;
use cmac::Cmac;
use core::cell::Ref;
use core::future::Future;
use embassy_time::Instant;
use heapless::Vec;
use p256::PublicKey;
use rand_core::{CryptoRng, RngCore};

// ------------------------------------------------------------------------
// Unprovisioned pipeline context
// ------------------------------------------------------------------------

impl<'a, E, N, S, R> UnprovisionedContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    fn rng_fill(&self, dest: &mut [u8]) {
        self.rng.borrow_mut().fill_bytes(dest);
    }

    type SetPeerPublicKeyFuture<'m> = impl Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_peer_public_key<'m>(&'m self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m> {
        async move { self.vault().set_peer_public_key(pk).await }
    }

    fn public_key(&self) -> Result<PublicKey, DeviceError> {
        self.vault().public_key()
    }

    type SetProvisioningDataFuture<'m> = impl Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_provisioning_data<'m>(
        &'m self,
        provisioning_salt: &'m [u8],
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m> {
        async move {
            self.vault()
                .set_provisioning_data(provisioning_salt, data)
                .await?;
            Ok(())
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
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        crypto::aes_ccm_decrypt_detached(key, nonce, data, mic, additional_data)
            .map_err(|_| DeviceError::CryptoError("aes_ccm_decrypt"))
    }

    fn rng_u8(&self) -> u8 {
        (self.rng.borrow_mut().next_u32() & 0xFF) as u8
    }

    fn rng_u32(&self) -> u32 {
        self.rng.borrow_mut().next_u32()
    }
}

impl<'a, E, N, S, R> MeshContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    fn uuid(&self) -> Uuid {
        self.vault().uuid()
    }

    fn network_retransmit(&self) -> NetworkRetransmitDetails {
        self.configuration_manager
            .configuration()
            .foundation_models()
            .configuration
            .network_transmit()
            .into()
    }

    type TransmitFuture<'m> = impl Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn transmit<'m>(&'m self, pdu: &'m PDU) -> Self::TransmitFuture<'m> {
        async move { Ok(self.network.transmit(pdu).await?) }
        /*
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
         */
    }

    fn primary_unicast_address(&self) -> Result<UnicastAddress, DeviceError> {
        if let Some(networks) = self.configuration_manager.configuration().network() {
            Ok(*networks.unicast_address())
        } else {
            Err(DeviceError::NotProvisioned)
        }
    }

    fn is_local_unicast(&self, addr: &Address) -> bool {
        if let Address::Unicast(addr) = addr {
            if let Ok(primary_addr) = self.primary_unicast_address() {
                if *addr >= primary_addr {
                    let element_index = *addr - primary_addr;
                    element_index < self.configuration_manager.composition().elements.len() as u8
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

// ------------------------------------------------------------------------
// Provisioned pipeline context
// ------------------------------------------------------------------------

impl<'a, E, N, S, R> ProvisionedContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
}

impl<'a, E, N, S, R> NetworkContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    fn network_deadline(&self, deadline: Option<Instant>) {
        self.deadline.borrow_mut().network(deadline)
    }
}

#[cfg(feature = "ble-mesh-relay")]
impl<'a, E, N, S, R> RelayContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    #[cfg(feature = "ble-mesh-relay")]
    fn is_relay_enabled(&self) -> bool {
        matches!(
            self.configuration_manager
                .configuration()
                .foundation_models()
                .configuration
                .relay()
                .relay,
            Relay::SupportedEnabled
        )
    }

    #[cfg(not(feature = "ble-mesh-relay"))]
    fn is_relay_enabled(&self) -> bool {
        false
    }

    #[cfg(feature = "ble-mesh-relay")]
    fn relay_retransmit(&self) -> NetworkRetransmitDetails {
        self.configuration_manager
            .configuration()
            .foundation_models()
            .configuration
            .relay()
            .into()
    }
}

impl<'a, E, N, S, R> AuthenticationContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    fn iv_index(&self) -> Option<u32> {
        self.vault().iv_index()
    }

    fn find_network_keys_by_nid(&self, nid: u8) -> Result<Vec<NetworkDetails, 10>, DeviceError> {
        if let Some(networks) = self.configuration_manager.configuration().network() {
            Ok(networks.find_by_nid(nid)?)
        } else {
            Err(DeviceError::NotProvisioned)
        }
    }
}

impl<'a, E, N, S, R> LowerContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    fn find_label_uuids_by_address(
        &self,
        addr: Address,
    ) -> Result<Option<Vec<LabelUuid, 3>>, DeviceError> {
        if let Address::Virtual(address) = addr {
            if let Some(network) = self.configuration_manager.configuration().network() {
                network
                    .subscriptions()
                    .find_label_uuids_by_address(address)
                    .map_err(|_| DeviceError::InsufficientBuffer)
                    .map(|inner| Some(inner))
            } else {
                Err(DeviceError::NotProvisioned)
            }
        } else {
            Ok(None)
        }
    }

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

    fn encrypt_application_key(
        &self,
        aid: ApplicationKeyIdentifier,
        nonce: ApplicationNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        self.vault()
            .encrypt_application_key(&aid, nonce, bytes, mic, additional_data)
    }

    fn decrypt_application_key(
        &self,
        aid: ApplicationKeyIdentifier,
        nonce: ApplicationNonce,
        bytes: &mut [u8],
        mic: &[u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        self.vault()
            .decrypt_application_key(&aid, nonce, bytes, mic, additional_data)
    }

    type NextSequenceFuture<'m> = impl Future<Output = Result<u32, DeviceError>> + 'm
    where
        Self: 'm;

    fn next_sequence<'m>(&'m self) -> Self::NextSequenceFuture<'m> {
        async move { self.configuration_manager.next_sequence().await }
    }

    fn default_ttl(&self) -> u8 {
        self.configuration_manager
            .configuration()
            .foundation_models()
            .configuration
            .default_ttl()
        //PrimaryElementStorage::retrieve(&self.configuration_manager)
        //.configuration
        //.default_ttl
    }

    fn has_any_subscription(&self, dst: &Address) -> bool {
        if let Some(network) = self.configuration_manager.configuration().network() {
            network.subscriptions().has_any_subscription(dst)
        } else {
            false
        }
    }

    fn is_locally_relevant(&self, dst: &Address) -> bool {
        self.is_local_unicast(dst) || self.has_any_subscription(dst)
    }

    fn ack_deadline(&self, deadline: Option<Instant>) {
        self.deadline.borrow_mut().ack(deadline);
    }
}

impl<'a, E, N, S, R> UpperContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    fn publish_deadline(&self, deadline: Option<Instant>) {
        self.deadline.borrow_mut().publish(deadline);
    }

    type RepublishFuture<'m> = impl Future<Output = ()> + 'm where Self: 'm;

    fn republish<'m>(&'m self, message: OutboundPublishMessage) -> Self::RepublishFuture<'m> {
        self.outbound.publish.send(message)
    }
}

impl<'a, E, N, S, R> AccessContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    type DispatchFuture<'m> = impl Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn dispatch_access<'m>(&'m self, message: &'m AccessMessage) -> Self::DispatchFuture<'m> {
        async move { self.elements.borrow_mut().dispatch(self, message).await }
    }
}

impl<'a, E, N, S, R> PipelineContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
}

impl<'a, E, N, S, R> ElementContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    R: CryptoRng + RngCore + 'a,
    S: Storage + 'a,
{
    type TransmitFuture<'m> = impl Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn transmit<'m>(&'m self, message: AccessMessage) -> Self::TransmitFuture<'m> {
        async move {
            self.outbound.access.send(message).await;
            Ok(())
        }
    }

    fn address(&self) -> Option<UnicastAddress> {
        if let Some(networks) = self.configuration_manager.configuration().network() {
            Some(*networks.unicast_address())
        } else {
            None
        }
    }
}

impl<'a, E, N, S, R> PrimaryElementContext for Node<'a, E, N, S, R>
where
    E: ElementsHandler<'a> + 'a,
    N: NetworkInterfaces + 'a,
    S: Storage + 'a,
    R: RngCore + CryptoRng + 'a,
{
    type NodeResetFuture<'m> = impl Future<Output = ()>
    where
        Self: 'm;

    fn node_reset<'m>(&'m self) -> Self::NodeResetFuture<'m> {
        async move { self.configuration_manager.node_reset().await }
    }

    fn composition(&self) -> &Composition {
        self.configuration_manager.composition()
    }

    fn configuration(&self) -> Ref<'_, Configuration> {
        self.configuration_manager.configuration()
    }

    type UpdateConfigurationFuture<'m, F> = impl Future<Output = Result<(), DeviceError>>
    where
        Self: 'm,
        F: 'm;

    fn update_configuration<F: FnOnce(&mut Configuration) -> Result<(), DeviceError>>(
        &self,
        update: F,
    ) -> Self::UpdateConfigurationFuture<'_, F> {
        self.configuration_manager.update_configuration(update)
    }

    fn is_local(&self, addr: &UnicastAddress) -> bool {
        self.is_local_unicast(&Address::Unicast(*addr))
    }
}
