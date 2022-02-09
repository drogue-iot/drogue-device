use core::convert::TryInto;
use core::future::Future;

use crate::drivers::ble::mesh::configuration_manager::{
    GeneralStorage, KeyStorage, NetworkInfo, NetworkKey, NetworkKeyDetails,
};
use crate::drivers::ble::mesh::crypto;
use aes::Aes128;
use cmac::crypto_mac::Output;
use cmac::Cmac;
use p256::elliptic_curve::ecdh::diffie_hellman;
use p256::PublicKey;

use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::crypto::nonce::DeviceNonce;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::ProvisioningData;
use heapless::Vec;

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

    fn network_keys(&self, nid: u8) -> Vec<NetworkKeyDetails, 10>;

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

    fn primary_unicast_address(&self) -> Option<UnicastAddress>;
}

pub struct StorageVault<'s, S: GeneralStorage + KeyStorage> {
    storage: &'s S,
}

impl<'s, S: GeneralStorage + KeyStorage> StorageVault<'s, S> {
    pub(crate) fn new(storage: &'s S) -> Self {
        Self { storage }
    }
}

impl<'s, S: GeneralStorage + KeyStorage> Vault for StorageVault<'s, S> {
    fn uuid(&self) -> Uuid {
        self.storage.uuid()
    }

    type SetPeerPublicKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_peer_public_key<'m>(&'m mut self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m> {
        async move {
            let mut keys = self.storage.retrieve();
            let secret_key = keys.private_key()?.ok_or(DeviceError::KeyInitialization)?;
            let shared_secret = diffie_hellman(secret_key.to_nonzero_scalar(), pk.as_affine());
            let _ = keys.set_shared_secret(Some(shared_secret));
            self.storage.store(keys).await
        }
    }

    fn public_key(&self) -> Result<PublicKey, DeviceError> {
        self.storage.retrieve().public_key()
    }

    fn n_k1(&self, salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        crypto::k1(
            self.storage
                .retrieve()
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

            let network_key = NetworkKeyDetails {
                network_key: NetworkKey::new(data.network_key),
                key_index: data.key_index,
                nid,
                encryption_key,
                privacy_key,
            };

            let mut network_keys = Vec::new();
            network_keys
                .push(network_key)
                .map_err(|_| DeviceError::InsufficientBuffer)?;

            let update = NetworkInfo {
                network_keys: network_keys,
                iv_update_flag: data.iv_update_flag,
                iv_index: data.iv_index,
                unicast_address: data.unicast_address,
            };

            defmt::info!("Assigned unicast address {:04x}", data.unicast_address);

            let mut keys = self.storage.retrieve();
            keys.set_network(&update);
            keys.set_provisioning_salt(
                provisioning_salt
                    .try_into()
                    .map_err(|_| DeviceError::InsufficientBuffer)?,
            )?;
            if let Some(salt) = keys.provisioning_salt()? {
                let device_key = self.prdk(&salt)?;
                let device_key = device_key.into_bytes();
                let device_key: [u8; 16] = device_key
                    .try_into()
                    .map_err(|_| DeviceError::KeyInitialization)?;
                keys.set_device_key(device_key);
            }
            self.storage.store(keys).await
        }
    }

    fn iv_index(&self) -> Option<u32> {
        if let Some(network) = self.storage.retrieve().network() {
            Some(network.iv_index)
        } else {
            None
        }
    }

    fn network_keys(&self, nid: u8) -> Vec<NetworkKeyDetails, 10> {
        if let Some(network) = self.storage.retrieve().network() {
            network
                .network_keys
                .iter()
                .filter(|e| e.nid == nid)
                .map(|e| e.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn is_local_unicast(&self, addr: &Address) -> bool {
        match addr {
            Address::Unicast(inner) => {
                if let Some(network) = self.storage.retrieve().network() {
                    let addr_bytes = network.unicast_address.to_be_bytes();
                    let addr = UnicastAddress::parse([addr_bytes[0], addr_bytes[1]]);
                    if let Ok(addr) = addr {
                        *inner == addr
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn decrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        let keys = self.storage.retrieve();
        if let Some(salt) = keys.provisioning_salt()? {
            let device_key = self.prdk(&salt)?;
            crypto::aes_ccm_decrypt_detached(
                &*device_key.into_bytes(),
                &nonce.into_bytes(),
                bytes,
                mic,
            )
            .map_err(|_| DeviceError::CryptoError)
        } else {
            Err(DeviceError::CryptoError)
        }
    }

    fn encrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
    ) -> Result<(), DeviceError> {
        let keys = self.storage.retrieve();
        if let Some(salt) = keys.provisioning_salt()? {
            let device_key = self.prdk(&salt)?.into_bytes();
            crypto::aes_ccm_encrypt_detached(&*device_key, &nonce.into_bytes(), bytes, mic)
                .map_err(|_| DeviceError::CryptoError)
        } else {
            Err(DeviceError::CryptoError)
        }
    }

    fn primary_unicast_address(&self) -> Option<UnicastAddress> {
        if let Some(network) = self.storage.retrieve().network() {
            network.unicast_address.try_into().ok()
        } else {
            None
        }
    }
}
