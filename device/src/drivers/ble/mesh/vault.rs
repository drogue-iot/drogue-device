use core::cell::Ref;
use core::convert::TryInto;
use core::future::Future;

use crate::drivers::ble::mesh::crypto;
use aes::Aes128;
use cmac::crypto_mac::Output;
use cmac::Cmac;
use p256::elliptic_curve::ecdh::diffie_hellman;
use p256::PublicKey;

use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::config::configuration_manager::ConfigurationManager;
use crate::drivers::ble::mesh::config::network::{Network, NetworkDetails};
use crate::drivers::ble::mesh::config::Configuration;
use crate::drivers::ble::mesh::crypto::nonce::{ApplicationNonce, DeviceNonce};
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::ProvisioningData;
use crate::drivers::ble::mesh::storage::Storage;

pub trait Vault {
    fn uuid(&self) -> Uuid;

    type SetPeerPublicKeyFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_peer_public_key<'m>(&'m mut self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m>;

    fn public_key(&self) -> Result<PublicKey, DeviceError>;

    fn aes_cmac(&self, key: &[u8], input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::aes_cmac(key, input).map_err(|_| DeviceError::InvalidKeyLength)
    }

    fn s1(input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::s1(input).map_err(|_| DeviceError::InvalidKeyLength)
    }

    fn k1(n: &[u8], salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::k1(n, salt, p).map_err(|_| DeviceError::InvalidKeyLength)
    }

    fn n_k1(&self, salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;

    fn prsk(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.n_k1(salt, b"prsk")
    }

    fn prsn(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.n_k1(salt, b"prsn")
    }

    fn prck(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.n_k1(salt, b"prck")
    }

    fn prdk(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.n_k1(salt, b"prdk")
    }

    fn k2(n: &[u8], p: &[u8]) -> Result<(u8, [u8; 16], [u8; 16]), DeviceError> {
        crypto::k2(n, p).map_err(|_| DeviceError::CryptoError)
    }

    fn aes_ccm_decrypt(
        key: &[u8],
        nonce: &[u8],
        data: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        crypto::aes_ccm_decrypt_detached(key, nonce, data, mic)
            .map_err(|_| DeviceError::CryptoError)
    }

    type SetProvisioningDataFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_provisioning_data<'m>(
        &'m mut self,
        provisioning_salt: &'m [u8],
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m>;

    fn iv_index(&self) -> Option<u32>;

    fn is_local_unicast(&self, addr: &Address) -> bool;

    fn decrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError>;

    fn encrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
    ) -> Result<(), DeviceError>;

    fn encrypt_application_key(
        &self,
        aid: &ApplicationKeyIdentifier,
        nonce: ApplicationNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    fn primary_unicast_address(&self) -> Option<UnicastAddress>;
}

pub struct StorageVault<'c, S: Storage> {
    configuration_manager: &'c ConfigurationManager<S>,
}

impl<'c, S: Storage> StorageVault<'c, S> {
    pub(crate) fn new(configuration_manager: &'c ConfigurationManager<S>) -> Self {
        Self {
            configuration_manager,
        }
    }

    fn config(&self) -> Ref<'_, Configuration> {
        self.configuration_manager.configuration()
    }
}

impl<'c, S: Storage> Vault for StorageVault<'c, S> {
    fn uuid(&self) -> Uuid {
        self.config().uuid().unwrap()
    }

    type SetPeerPublicKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_peer_public_key<'m>(&'m mut self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m> {
        async move {
            self.configuration_manager
                .update_configuration(|config| {
                    let secret_key = config
                        .device_keys()
                        .private_key()?
                        .ok_or(DeviceError::KeyInitialization)?;
                    let shared_secret =
                        diffie_hellman(secret_key.to_nonzero_scalar(), pk.as_affine());
                    config
                        .device_keys_mut()
                        .set_shared_secret(Some(shared_secret))?;
                    Ok(())
                })
                .await
        }
    }

    fn public_key(&self) -> Result<PublicKey, DeviceError> {
        self.configuration_manager
            .configuration()
            .device_keys()
            .public_key()
    }

    fn n_k1(&self, salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::k1(
            self.configuration_manager
                .configuration()
                .device_keys()
                .shared_secret()?
                .ok_or(DeviceError::CryptoError)?
                .as_bytes(),
            salt,
            p,
        )
        .map_err(|_| DeviceError::CryptoError)
    }

    type SetProvisioningDataFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_provisioning_data<'m>(
        &'m mut self,
        provisioning_salt: &'m [u8],
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m> {
        async move {
            let (nid, encryption_key, privacy_key) = crypto::k2(&data.network_key, &[0x00])
                .map_err(|_| DeviceError::KeyInitialization)?;

            let primary_network_details = NetworkDetails::new(
                data.network_key.into(),
                data.key_index,
                nid,
                encryption_key,
                privacy_key,
            );

            self.configuration_manager
                .update_configuration(|config| {
                    let device_key = self.prdk(&provisioning_salt)?;
                    let device_key = device_key.into_bytes();
                    let device_key: [u8; 16] = device_key
                        .try_into()
                        .map_err(|_| DeviceError::KeyInitialization)?;
                    config.device_keys_mut().set_device_key(device_key);

                    config.network_mut().replace(Network::new(
                        primary_network_details,
                        data.iv_update_flag,
                        data.iv_index,
                        data.unicast_address,
                    ));

                    Ok(())
                })
                .await?;

            info!("Assigned unicast address {:04x}", data.unicast_address);
            Ok(())
        }
    }

    fn iv_index(&self) -> Option<u32> {
        if let Some(network) = self.configuration_manager.configuration().network() {
            Some(network.iv_index())
        } else {
            None
        }
    }

    fn is_local_unicast(&self, addr: &Address) -> bool {
        self.configuration_manager.is_local_unicast(addr)
    }

    fn decrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        let device_key = self
            .config()
            .device_keys()
            .device_key()
            .ok_or(DeviceError::CryptoError)?;
        crypto::aes_ccm_decrypt_detached(device_key.as_ref(), &*nonce, bytes, mic)
            .map_err(|_| DeviceError::CryptoError)
    }

    fn encrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
    ) -> Result<(), DeviceError> {
        let device_key = self
            .config()
            .device_keys()
            .device_key()
            .ok_or(DeviceError::CryptoError)?;
        crypto::aes_ccm_encrypt_detached(device_key.as_ref(), &*nonce, bytes, mic, None)
            .map_err(|_| DeviceError::CryptoError)
    }

    fn encrypt_application_key(
        &self,
        aid: &ApplicationKeyIdentifier,
        nonce: ApplicationNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        if let Some(network) = self.config().network() {
            if let Some(app_key) = network.find_app_key_by_aid(aid) {
                return crypto::aes_ccm_encrypt_detached(
                    app_key.key.as_ref(),
                    &*nonce,
                    bytes,
                    mic,
                    additional_data,
                )
                .map_err(|_| DeviceError::CryptoError);
            }
        }

        Err(DeviceError::CryptoError)
    }

    fn primary_unicast_address(&self) -> Option<UnicastAddress> {
        if let Some(network) = self.config().network() {
            Some(*network.unicast_address())
        } else {
            None
        }
    }
}
