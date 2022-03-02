pub(crate) mod app_keys;
pub(crate) mod bindings;
pub(crate) mod configuration_manager;
pub(crate) mod device_keys;
pub(crate) mod foundation_models;
pub(crate) mod network;
pub(crate) mod publications;
pub(crate) mod subcriptions;

use crate::drivers::ble::mesh::composition::Composition;
use crate::drivers::ble::mesh::config::configuration_manager::SEQUENCE_THRESHOLD;
use crate::drivers::ble::mesh::config::device_keys::DeviceKeys;
use crate::drivers::ble::mesh::config::foundation_models::FoundationModels;
use crate::drivers::ble::mesh::config::network::Network;
use crate::drivers::ble::mesh::device::Uuid;
use p256::SecretKey;
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Configuration {
    seq: u32,
    uuid: Option<Uuid>,
    device_keys: DeviceKeys,
    network: Option<Network>,
    foundation_models: FoundationModels,
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

        if let Ok(None) = self.device_keys.private_key() {
            let secret_key = SecretKey::random(rng);
            let _ = self.device_keys_mut().set_private_key(&Some(secret_key));
            changed = true;
        }

        if self.seq % SEQUENCE_THRESHOLD == 0 {
            self.seq = self.seq + SEQUENCE_THRESHOLD;
            changed = true;
        }

        changed
    }

    #[cfg(feature = "defmt")]
    fn display_configuration(&self, composition: &Composition) {
        if let Some(uuid) = self.uuid {
            info!("UUID: {}", uuid);
        } else {
            info!("UUID: not set");
        }
        self.device_keys.display_configuration();
        if let Some(network) = &self.network {
            network.display_configuration(composition);
        }
    }

    pub fn uuid(&self) -> &Option<Uuid> {
        &self.uuid
    }

    pub fn device_keys(&self) -> &DeviceKeys {
        &self.device_keys
    }

    pub fn device_keys_mut(&mut self) -> &mut DeviceKeys {
        &mut self.device_keys
    }

    pub fn network(&self) -> &Option<Network> {
        &self.network
    }

    pub fn network_mut(&mut self) -> &mut Option<Network> {
        &mut self.network
    }

    pub fn foundation_models(&self) -> &FoundationModels {
        &self.foundation_models
    }

    pub fn foundation_models_mut(&mut self) -> &mut FoundationModels {
        &mut self.foundation_models
    }
}
