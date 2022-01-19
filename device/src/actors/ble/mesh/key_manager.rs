use crate::actors::ble::mesh::configuration_manager::{KeyStorage, Keys, Network};
use crate::actors::ble::mesh::device::{DeviceError, RandomProvider};
use crate::drivers::ble::mesh::crypto::{k1, k2};
use crate::drivers::ble::mesh::provisioning::ProvisioningData;
use crate::drivers::ble::mesh::storage::{Payload, Storage};
use crate::drivers::ble::mesh::transport::Transport;
use aes::Aes128;
use cmac::crypto_mac::{InvalidKeyLength, Output};
use cmac::Cmac;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use p256::elliptic_curve::ecdh::{diffie_hellman, SharedSecret};
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::{AffinePoint, EncodedPoint, NistP256, PublicKey, SecretKey};
use rand_core::{CryptoRng, Error, RngCore};

pub struct KeyManager<R, S>
where
    R: CryptoRng + RngCore + 'static,
    S: KeyStorage + RandomProvider<R> + 'static,
{
    services: Option<UnsafeCell<*const S>>,
    private_key: Option<SecretKey>,
    peer_public_key: Option<PublicKey>,
    shared_secret: Option<SharedSecret<NistP256>>,
    _marker: PhantomData<(R, S)>,
}

impl<R, S> KeyManager<R, S>
where
    R: CryptoRng + RngCore + 'static,
    S: KeyStorage + RandomProvider<R> + 'static,
{
    pub fn new() -> Self {
        Self {
            services: None,
            private_key: None,
            peer_public_key: None,
            shared_secret: None,
            _marker: PhantomData,
        }
    }

    fn load_keys(&mut self) -> Result<(), DeviceError> {
        let keys = self.services()?.retrieve();
        self.private_key = keys
            .private_key()
            .map_err(|_| DeviceError::KeyInitialization)?;
        self.shared_secret = keys
            .shared_secret()
            .map_err(|_| DeviceError::KeyInitialization)?;
        Ok(())
    }

    async fn store_keys(&mut self) -> Result<(), DeviceError> {
        self.update_stored( |manager, keys| {
            keys.set_private_key(&manager.private_key);
            keys.set_shared_secret(&manager.shared_secret);
            Ok(())
        }).await
    }

    fn set_services(&mut self, services: *const S) {
        self.services.replace(UnsafeCell::new(services));
    }

    fn services(&self) -> Result<&S, DeviceError> {
        match &self.services {
            None => Err(DeviceError::NoServices),
            Some(services) => Ok(unsafe { &**services.get() }),
        }
    }

    pub(crate) async fn initialize(&mut self, services: *const S) -> Result<(), DeviceError> {
        self.set_services(services);
        self.load_keys()?;

        if let None = self.private_key {
            defmt::info!("** Generating secrets");
            let secret = SecretKey::random(&mut *self.services()?.rng());
            self.private_key.replace(secret);
            defmt::info!("   ...complete");
            self.store_keys().await?
        }
        Ok(())
    }

    pub fn public_key(&self) -> Result<PublicKey, DeviceError> {
        match &self.private_key {
            None => Err(DeviceError::KeyInitialization),
            Some(private_key) => Ok(private_key.public_key()),
        }
    }

    pub async fn set_peer_public_key(&mut self, pk: PublicKey) -> Result<(), DeviceError> {
        match &self.private_key {
            None => return Err(DeviceError::KeyInitialization),
            Some(private_key) => {
                self.shared_secret.replace(diffie_hellman(
                    private_key.to_nonzero_scalar(),
                    pk.as_affine(),
                ));
                self.store_keys().await?;
            }
        }
        self.peer_public_key.replace(pk);
        Ok(())
    }

    pub async fn set_provisioning_data(
        &mut self,
        data: &ProvisioningData,
    ) -> Result<(), DeviceError> {
        defmt::info!("******************************** SET PROVISIONING DATA");
        self.update_stored( |_manager, keys| {

            let (nid, encryption_key, privacy_key) = k2(&data.network_key, &[0x00]).map_err(|_|DeviceError::KeyInitialization)?;

            let update = Network {
                network_key: data.network_key,
                key_index: data.key_index,
                key_refresh_flag: data.key_refresh_flag,
                iv_update_flag: data.iv_update_flag,
                iv_index: data.iv_index,
                unicast_address: data.unicast_address,
                nid,
                encryption_key,
                privacy_key,
            };
            keys.set_network(&update);
            Ok(())
        }).await
    }

    async fn update_stored<F>(&mut self, update: F) -> Result<(), DeviceError>
    where
        F: FnOnce(&mut Self, &mut Keys) -> Result<(), DeviceError>,
    {
        let mut keys = self.services()?.retrieve();
        update(self, &mut keys)?;
        self.services()?
            .store(keys)
            .await
            .map_err(|_| DeviceError::KeyInitialization)
    }

    pub fn k1(&self, salt: &[u8], p: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError> {
        Ok(k1(
            self.shared_secret
                .as_ref()
                .ok_or(DeviceError::NoSharedSecret)?
                .as_bytes(),
            salt,
            p,
        )?)
    }
}
