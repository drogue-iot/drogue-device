use core::convert::TryInto;
use core::future::Future;

use aes::cipher::generic_array::GenericArray;
use aes::{Aes128, NewBlockCipher};
use ccm::aead::NewAead;
use ccm::aead::{AeadInPlace, Buffer};
use ccm::consts::U13;
use ccm::consts::U8;
use ccm::Ccm;
use cmac::crypto_mac::{InvalidKeyLength, Output};
use cmac::{Cmac, Mac, NewMac};
use heapless::Vec;
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::ecdh::diffie_hellman;
use p256::{PublicKey, SecretKey};
use rand_core::{CryptoRng, RngCore};

use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::ProvisioningData;

type AesCcm = Ccm<Aes128, U8, U13>;

pub trait Vault {
    fn uuid(&self) -> Uuid;

    type PeerPublicKeyFuture<'m>: Future<Output = Result<Option<PublicKey>, DeviceError>>
    where
        Self: 'm;

    fn peer_public_key<'m>(&'m self) -> Self::PeerPublicKeyFuture<'m>;

    type SetPeerPublicKeyFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_peer_public_key<'m>(&'m mut self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m>;

    fn public_key(&self) -> Result<PublicKey, DeviceError>;

    fn aes_cmac(&self, key: &[u8], input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        let mut mac = Cmac::<Aes128>::new_from_slice(key)?;
        mac.update(input);
        Ok(mac.finalize())
    }

    const ZERO: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    fn s1(&self, input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.aes_cmac(&Self::ZERO, input)
    }

    fn k1(&self, n: &[u8], salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        let t = self.aes_cmac(&salt, n)?;
        let t = t.into_bytes();
        self.aes_cmac(&t, p)
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

    fn k2(&self, n: &[u8], p: &[u8]) -> Result<(u8, [u8; 16], [u8; 16]), DeviceError> {
        let salt = self.s1(b"smk2")?;
        let t = &self.aes_cmac(&salt.into_bytes(), n)?.into_bytes();

        let mut input: Vec<u8, 64> = Vec::new();
        input
            .extend_from_slice(p)
            .map_err(|_| DeviceError::InvalidKeyLength)?;
        input.push(0x01);
        let t1 = &self.aes_cmac(t, &input)?.into_bytes();

        let nid = t1[15] & 0x7F;
        defmt::info!("NID {:x}", nid);

        input.truncate(0);
        input
            .extend_from_slice(&t1)
            .map_err(|_| DeviceError::InvalidKeyLength)?;
        input
            .extend_from_slice(p)
            .map_err(|_| DeviceError::InvalidKeyLength)?;
        input.push(0x02);

        let t2 = self.aes_cmac(t, &input)?.into_bytes();

        let encryption_key = t2;

        input.truncate(0);
        input
            .extend_from_slice(&t2)
            .map_err(|_| DeviceError::InvalidKeyLength)?;
        input
            .extend_from_slice(p)
            .map_err(|_| DeviceError::InvalidKeyLength)?;
        input.push(0x03);

        let t3 = self.aes_cmac(t, &input)?.into_bytes();
        let privacy_key = t3;

        Ok((
            nid,
            encryption_key
                .try_into()
                .map_err(|_| DeviceError::InvalidKeyLength)?,
            privacy_key
                .try_into()
                .map_err(|_| DeviceError::InvalidKeyLength)?,
        ))
    }

    fn aes_ccm_decrypt(
        &self,
        key: &[u8],
        nonce: &[u8],
        data: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError> {
        let key = GenericArray::<u8, <Aes128 as NewBlockCipher>::KeySize>::from_slice(key);
        let ccm = AesCcm::new(&key);
        ccm.decrypt_in_place_detached(nonce.into(), &[], data, mic.into())
            .map_err(|_| DeviceError::CryptoError)
    }

    type SetProvisioningDataFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_provisioning_data<'m>(
        &mut self,
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m>;
}

pub struct InMemoryVault {
    uuid: Uuid,
    secret_key: SecretKey,
    peer_public_key: Option<PublicKey>,
    //
    shared_secret: Option<SharedSecret>,
}

impl InMemoryVault {
    pub fn new<R: RngCore + CryptoRng>(uuid: Uuid, rng: &mut R) -> Self {
        let secret_key = SecretKey::random(rng);

        Self {
            uuid,
            secret_key,
            peer_public_key: None,
            //
            shared_secret: None,
        }
    }
}

impl Vault for InMemoryVault {
    fn uuid(&self) -> Uuid {
        self.uuid
    }

    type PeerPublicKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<Option<PublicKey>, DeviceError>>;

    fn peer_public_key<'m>(&'m self) -> Self::PeerPublicKeyFuture<'m> {
        async move { Ok(self.peer_public_key) }
    }

    type SetPeerPublicKeyFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_peer_public_key<'m>(&'m mut self, pk: PublicKey) -> Self::SetPeerPublicKeyFuture<'m> {
        async move {
            self.peer_public_key.replace(pk);
            self.shared_secret.replace(diffie_hellman(
                self.secret_key.to_nonzero_scalar(),
                pk.as_affine(),
            ));
            Ok(())
        }
    }

    fn public_key(&self) -> Result<PublicKey, DeviceError> {
        Ok(self.secret_key.public_key())
    }

    fn n_k1(&self, salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        self.k1(
            self.shared_secret
                .as_ref()
                .ok_or(DeviceError::KeyInitialization)?
                .as_bytes(),
            salt,
            p,
        )
    }

    type SetProvisioningDataFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn set_provisioning_data<'m>(
        &mut self,
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m> {
        async move {
            defmt::info!("PROVISIONED");
            Ok(())
        }
    }
}
