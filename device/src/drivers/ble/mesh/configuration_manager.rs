use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::IVUpdateFlag;
use crate::drivers::ble::mesh::storage::{Payload, Storage};
use core::cell::RefCell;
use core::convert::TryInto;
use defmt::{Format, Formatter};
use futures::future::Future;
use heapless::Vec;
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::generic_array::{typenum::consts::U32, GenericArray};
use p256::{PublicKey, SecretKey};
use postcard::{from_bytes, to_slice};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyIndex, NetKeyIndex};
use crate::drivers::ble::mesh::model::Status;

const SEQUENCE_THRESHOLD: u32 = 100;

#[derive(Serialize, Deserialize, Clone, Default, Format)]
pub struct Configuration {
    seq: u32,
    uuid: Option<Uuid>,
    keys: Keys,
    primary: PrimaryElementModels,
}

impl Configuration {
    fn validate<R: CryptoRng + RngCore>(&mut self, rng: &mut R) -> bool {
        let mut changed = false;

        if self.uuid.is_none() {
            let mut uuid = [0; 16];
            rng.fill_bytes(&mut uuid);
            self.uuid.replace(Uuid(
                *uuid::Builder::from_random_bytes(uuid)
                    .into_uuid()
                    .as_bytes(),
            ));
            changed = true;
        }

        if let Ok(None) = self.keys.private_key() {
            let secret_key = SecretKey::random(rng);
            let _ = self.keys.set_private_key(&Some(secret_key));
            changed = true;
        }

        if self.seq % SEQUENCE_THRESHOLD == 0 {
            self.seq = self.seq + SEQUENCE_THRESHOLD;
            changed = true;
        }

        changed
    }

    fn display_configuration(&self) {
        if let Some(uuid) = self.uuid {
            defmt::info!("UUID: {}", uuid);
        } else {
            defmt::info!("UUID: not set");
        }
        self.keys.display_configuration();
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct DeviceKey([u8; 16]);

impl DeviceKey {
    pub fn new(material: [u8; 16]) -> Self {
        Self(material)
    }
}

impl Format for DeviceKey {
    fn format(&self, fmt: Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Format)]
pub struct Keys {
    random: Option<[u8; 16]>,
    private_key: Option<[u8; 32]>,
    shared_secret: Option<[u8; 32]>,
    provisioning_salt: Option<[u8; 16]>,
    device_key: Option<DeviceKey>,
    network: Option<NetworkInfo>,
}

#[derive(Serialize, Deserialize, Clone, Format)]
pub struct NetworkInfo {
    pub(crate) network_keys: Vec<NetworkKeyStorage, 10>,
    pub(crate) iv_update_flag: IVUpdateFlag,
    pub(crate) iv_index: u32,
    pub(crate) unicast_address: UnicastAddress,
}

impl NetworkInfo {
    fn display_configuration(&self) {
        defmt::info!("Primary unicast address: {}", self.unicast_address);
        defmt::info!("IV index: {:x}", self.iv_index);

        for key in &self.network_keys {
            key.display_configuration();
        }
    }

    fn by_index(&self, net_key_index: NetKeyIndex) -> Option<&NetworkKeyStorage> {
        self.network_keys.iter().find(|e| e.key_index == net_key_index)
    }

    fn by_index_mut(&mut self, net_key_index: NetKeyIndex) -> Option<&mut NetworkKeyStorage> {
        self.network_keys.iter_mut().find(|e| e.key_index == net_key_index)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct NetworkKey([u8; 16]);

impl NetworkKey {
    pub fn new(material: [u8; 16]) -> Self {
        Self(material)
    }
}

impl Format for NetworkKey {
    fn format(&self, fmt: Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Format)]
pub struct NetworkKeyStorage {
    pub(crate) network_key: NetworkKey,
    pub(crate) key_index: NetKeyIndex,
    pub(crate) nid: u8,
    pub(crate) encryption_key: [u8; 16],
    pub(crate) privacy_key: [u8; 16],
    pub(crate) app_keys: Vec<AppKeyDetails, 10>,
}

impl NetworkKeyStorage {
    fn display_configuration(&self) {
        defmt::info!("Network Keys");
        defmt::info!("  {}: {} [nid={}]", self.key_index, self.network_key, self.nid);
        defmt::info!("Application Keys:");
        for app_key in &self.app_keys {
            app_key.display_configuration()
        }
    }

    fn add_app_key(&mut self, app_key_index: AppKeyIndex, app_key: [u8; 16]) -> Result<(), Status> {
        if let Some(_) = self.app_keys.iter().find(|e| e.key_index == app_key_index ) {
            Err(Status::KeyIndexAlreadyStored)
        } else {
            self.app_keys.push( AppKeyDetails {
                app_key: AppKey( app_key ),
                key_index: app_key_index,
            }).map_err(|_|Status::InsufficientResources)?;
            Ok(())
        }
    }

    fn app_key_indexes(&self) -> Vec<AppKeyIndex, 10> {
        self.app_keys.iter().map(|e|e.key_index).collect()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Format)]
pub struct NetworkKeyHandle {
    pub(crate) network_key: NetworkKey,
    pub(crate) key_index: NetKeyIndex,
    pub(crate) nid: u8,
    pub(crate) encryption_key: [u8; 16],
    pub(crate) privacy_key: [u8; 16],
}

impl From<NetworkKeyStorage> for NetworkKeyHandle {
    fn from(key: NetworkKeyStorage) -> Self {
        Self {
            network_key: key.network_key,
            key_index: key.key_index,
            nid: key.nid,
            encryption_key: key.encryption_key,
            privacy_key: key.privacy_key,
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct AppKey(pub(crate) [u8; 16]);

impl Format for AppKey {
    fn format(&self, fmt: Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Format)]
pub struct AppKeyDetails {
    pub(crate) app_key: AppKey,
    pub(crate) key_index: AppKeyIndex,
}

impl AppKeyDetails {
    pub(crate) fn display_configuration(&self) {
        defmt::info!("  {}: {}", self.key_index, self.app_key)
    }
}

impl Keys {
    fn display_configuration(&self) {
        if let Some(key) = self.device_key {
            defmt::info!("DeviceKey: {}", key);
        } else {
            defmt::info!("DeviceKey: None");
        }

        if let Some(network) = &self.network {
            network.display_configuration();
        }
    }

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
        Ok(self
            .private_key()?
            .ok_or(DeviceError::KeyInitialization)?
            .public_key())
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

    pub(crate) fn network(&self) -> &Option<NetworkInfo> {
        &self.network
    }

    pub(crate) fn set_network(&mut self, network: &NetworkInfo) {
        self.network.replace(network.clone());
    }

    pub(crate) fn set_provisioning_salt(
        &mut self,
        provisioning_salt: [u8; 16],
    ) -> Result<(), DeviceError> {
        self.provisioning_salt.replace(provisioning_salt);
        Ok(())
    }

    pub(crate) fn provisioning_salt(&self) -> Result<Option<[u8; 16]>, DeviceError> {
        Ok(self.provisioning_salt)
    }

    pub(crate) fn set_device_key(&mut self, key: [u8; 16]) {
        self.device_key.replace(DeviceKey::new(key));
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

pub trait PrimaryElementStorage {
    type StoreFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn store<'m>(&'m self, element: PrimaryElementModels) -> Self::StoreFuture<'m>;

    fn retrieve(&self) -> PrimaryElementModels;
}

pub struct ConfigurationManager<S: Storage> {
    storage: RefCell<S>,
    config: RefCell<Configuration>,
    runtime_seq: RefCell<u32>,
    force_reset: bool,
}

impl<S: Storage> GeneralStorage for ConfigurationManager<S> {
    fn uuid(&self) -> Uuid {
        self.config.borrow().uuid.unwrap()
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
        self.config.borrow().keys.clone()
    }
}

impl<S: Storage> PrimaryElementStorage for ConfigurationManager<S> {
    type StoreFuture<'m>
    where
        Self: 'm,
    = impl Future<Output = Result<(), DeviceError>>;

    fn store<'m>(&'m self, primary: PrimaryElementModels) -> Self::StoreFuture<'m> {
        let mut update = self.config.borrow().clone();
        update.primary = primary;
        async move { self.store(&update).await }
    }

    fn retrieve(&self) -> PrimaryElementModels {
        self.config.borrow().primary.clone()
    }
}

impl<S: Storage> ConfigurationManager<S> {
    pub fn new(storage: S, force_reset: bool) -> Self {
        Self {
            storage: RefCell::new(storage),
            config: RefCell::new(Default::default()),
            force_reset,
            runtime_seq: RefCell::new(0),
        }
    }

    pub(crate) async fn initialize<R: RngCore + CryptoRng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), DeviceError> {
        if self.force_reset {
            defmt::info!("Performing FORCE RESET");
            let mut config = Configuration::default();
            config.validate(rng);
            self.store(&config).await
        } else {
            let payload = self
                .storage
                .borrow_mut()
                .retrieve()
                .await
                .map_err(|_| DeviceError::StorageInitialization)?;
            match payload {
                None => {
                    defmt::info!("error loading configuration");
                    Err(DeviceError::StorageInitialization)
                }
                Some(payload) => {
                    let mut config: Configuration =
                        from_bytes(&payload.payload).map_err(|_| DeviceError::Serialization)?;
                    if config.validate(rng) {
                        // we initialized some things that we should stuff away.
                        self.runtime_seq.replace(config.seq);
                        self.store(&config).await?;
                    } else {
                        self.runtime_seq.replace(config.seq);
                        self.config.replace(config);
                    }
                    Ok(())
                }
            }
        }
    }

    pub(crate) async fn node_reset(&self) -> ! {
        // best effort
        self.store(&Configuration::default()).await.ok();
        // todo don't assume cortex-m some day
        cortex_m::peripheral::SCB::sys_reset();
    }

    pub(crate) fn display_configuration(&self) {
        defmt::info!("================================================================");
        defmt::info!("Message Sequence: {}", *self.runtime_seq.borrow());
        self.config.borrow().display_configuration();
        defmt::info!("================================================================");
    }

    fn retrieve(&self) -> Configuration {
        self.config.borrow().clone()
    }

    async fn store(&self, config: &Configuration) -> Result<(), DeviceError> {
        let mut payload = [0; 512];
        to_slice(config, &mut payload)?;
        let payload = Payload { payload };
        self.storage
            .borrow_mut()
            .store(&payload)
            .await
            .map_err(|_| DeviceError::Storage)?;
        self.config.replace(config.clone());
        Ok(())
    }

    pub(crate) async fn next_sequence(&self) -> Result<u32, DeviceError> {
        let mut runtime_seq = self.runtime_seq.borrow_mut();
        let seq = *runtime_seq;
        *runtime_seq = *runtime_seq + 1;
        if *runtime_seq % SEQUENCE_THRESHOLD == 0 {
            let mut config = self.retrieve();
            config.seq = *runtime_seq;
            self.store(&config).await?;
        }
        Ok(seq)
    }

    pub(crate) fn reset(&mut self) {
        self.force_reset = true;
    }

    pub(crate) async fn add_app_key(&self, net_key_index: NetKeyIndex, app_key_index: AppKeyIndex, app_key: [u8;16]) -> Result<Status, DeviceError> {
        let mut config = self.retrieve();
        if let Some(ref mut network) = config.keys.network {
            if let Some(specific_network) = network.by_index_mut(net_key_index) {
                let result = specific_network.add_app_key( app_key_index, app_key );
                match result {
                    Ok(_) => {
                        self.store(&config).await?;
                        Ok(Status::Success)
                    }
                    Err(status) => {
                        Ok(status)
                    }
                }
            } else {
                Ok(Status::InvalidNetKeyIndex)
            }
        } else {
            Err(DeviceError::NotProvisioned)
        }
    }

    pub fn app_key_indexes(&self, net_key_index: NetKeyIndex) -> Result<Vec<AppKeyIndex, 10>, Status> {
        if let Some(network) = self.retrieve().keys.network {
            if let Some(specific_network) = network.by_index(net_key_index) {
                Ok(specific_network.app_key_indexes())
            } else {
                Err(Status::InvalidNetKeyIndex)
            }
        } else {
            Err(Status::InvalidNetKeyIndex)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Format)]
pub struct PrimaryElementModels {
    pub(crate) configuration: ConfigurationModel,
}

#[derive(Serialize, Deserialize, Clone, Format)]
pub struct ConfigurationModel {
    pub(crate) secure_beacon: bool,
    pub(crate) default_ttl: u8,
}

impl Default for ConfigurationModel {
    fn default() -> Self {
        Self {
            secure_beacon: true,
            default_ttl: 127,
        }
    }
}
