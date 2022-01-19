use crate::drivers::ble::mesh::storage::{Payload, Storage};
use core::cell::RefCell;
use core::convert::TryInto;
use defmt::Format;
use futures::future::Future;
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::{AffinePoint, EncodedPoint, PublicKey, SecretKey};
use p256::elliptic_curve::AffineXCoordinate;
use p256::elliptic_curve::generic_array::{
    GenericArray,
    typenum::consts::U32,
};
use p256::elliptic_curve::group::GroupEncoding;
use postcard::{from_bytes, to_slice};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::{IVUpdateFlag, KeyRefreshFlag, ProvisioningData};

#[derive(Serialize, Deserialize, Copy, Clone, Default, Format)]
pub struct Configuration {
    uuid: Option<[u8; 16]>,
    keys: Keys,
}

impl Configuration {
    fn validate<R: CryptoRng + RngCore>(&mut self, rng: &mut R) -> bool {
        let mut changed = false;

        if self.uuid.is_none() {
            let mut uuid = [0; 16];
            rng.fill_bytes(&mut uuid);
            self.uuid.replace(uuid);
            changed = true;
        }

        if let Ok(None) = self.keys.private_key() {
            let secret_key = SecretKey::random(rng);
            self.keys.set_private_key(&Some(secret_key));
            changed = true;
        }

        changed
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default, Format)]
pub struct Keys {
    random: Option<[u8; 16]>,
    private_key: Option<[u8; 32]>,
    shared_secret: Option<[u8; 32]>,
    network: Option<Network>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Default, Format)]
pub struct Network {
    pub(crate) network_key: [u8; 16],
    pub(crate) key_index: u16,
    pub(crate) key_refresh_flag: KeyRefreshFlag,
    pub(crate) iv_update_flag: IVUpdateFlag,
    pub(crate) iv_index: u32,
    pub(crate) unicast_address: u16,
    // derived attributes
    pub(crate) nid: u8,
    pub(crate) encryption_key: [u8; 16],
    pub(crate) privacy_key: [u8; 16],
}

impl Keys {
    pub(crate) fn private_key(&self) -> Result<Option<SecretKey>, DeviceError> {
        match self.private_key {
            None => Ok(None),
            Some(private_key) => Ok(Some(
                SecretKey::from_be_bytes(&private_key).map_err(|_| DeviceError::Serialization)?,
            )),
        }
    }

    fn set_private_key(&mut self, private_key: &Option<SecretKey>) -> Result<(), DeviceError> {
        match private_key {
            None => {
                self.private_key.take();
            }
            Some(private_key) => {
                self.private_key.replace(
                    private_key
                        .to_nonzero_scalar()
                        .to_bytes()
                        .try_into()
                        .map_err(|_| DeviceError::Serialization)?,
                );
            }
        }
        Ok(())
    }

    pub(crate) fn public_key(&self) -> Result<PublicKey, DeviceError> {
        Ok(self.private_key()?.ok_or(DeviceError::KeyInitialization)?.public_key())
    }

    pub(crate) fn shared_secret(&self) -> Result<Option<SharedSecret>, DeviceError> {
        match self.shared_secret {
            None => Ok(None),
            Some(shared_secret) => {
                let arr: GenericArray<u8, U32> = shared_secret.into();
                Ok(Some(SharedSecret::from(arr)))
            }
        }
    }

    pub(crate) fn set_shared_secret(
        &mut self,
        shared_secret: Option<SharedSecret>,
    ) -> Result<(), ()> {
        match shared_secret {
            None => {
                self.shared_secret.take();
            }
            Some(shared_secret) => {
                let bytes = &shared_secret.as_bytes()[0..];
                self.shared_secret
                    .replace(bytes.try_into().map_err(|_| ())?);
            }
        }
        Ok(())
    }

    pub(crate) fn network(&self) -> Option<Network> {
        self.network
    }

    pub(crate) fn set_network(&mut self, network: &Network) -> Result<(),()> {
        self.network.replace(*network);
        Ok(())
    }
}

pub trait GeneralStorage {
    fn uuid(&self) -> Uuid;
}

pub trait KeyStorage {
    type StoreFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn store<'m>(&'m self, keys: Keys) -> Self::StoreFuture<'m>;

    fn retrieve<'m>(&'m self) -> Keys;
}

pub struct ConfigurationManager<S: Storage> {
    storage: RefCell<S>,
    config: RefCell<Configuration>,
}

impl<S: Storage> GeneralStorage for ConfigurationManager<S> {
    fn uuid(&self) -> Uuid {
        Uuid(self.config.borrow().uuid.unwrap())
    }
}

impl<S: Storage> KeyStorage for ConfigurationManager<S> {
    type StoreFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn store<'m>(&'m self, keys: Keys) -> Self::StoreFuture<'m> {
        let mut update = self.config.borrow().clone();
        update.keys = keys;
        async move { self.store(&update).await }
    }

    fn retrieve<'m>(&'m self) -> Keys {
        self.config.borrow().keys
    }
}

impl<S: Storage> ConfigurationManager<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage: RefCell::new(storage),
            config: RefCell::new(Default::default()),
        }
    }

    pub(crate) async fn initialize<R: RngCore + CryptoRng>(&mut self, rng: &mut R) -> Result<(), DeviceError> {
        let payload = self.storage.borrow_mut().retrieve().await.map_err(|_|DeviceError::StorageInitialization)?;
        match payload {
            None => Err(DeviceError::StorageInitialization),
            Some(payload) => {
                let mut config: Configuration = from_bytes(&payload.payload).map_err(|_| DeviceError::Serialization)?;
                if config.validate(rng) {
                    // we initialized some things that we should stuff away.
                    self.store(&config).await?;
                } else {
                    self.config.replace(config);
                }
                defmt::info!("Load {}", &*self.config.borrow());
                Ok(())
            }
        }
    }

    fn retrieve(&self) -> Configuration {
        *self.config.borrow()
    }

    async fn store(&self, config: &Configuration) -> Result<(), DeviceError> {
        defmt::info!("Store {}", config);
        let mut payload = [0; 512];
        to_slice(config, &mut payload)?;
        let payload = Payload { payload };
        self.storage.borrow_mut().store(&payload).await.map_err(|_|DeviceError::Storage)?;
        self.config.replace(*config);
        Ok(())
    }
}
