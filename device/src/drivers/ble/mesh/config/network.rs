use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
#[cfg(feature = "defmt")]
use crate::drivers::ble::mesh::composition::Composition;
use crate::drivers::ble::mesh::config::app_keys::AppKeyDetails;
use crate::drivers::ble::mesh::config::bindings::Bindings;
use crate::drivers::ble::mesh::config::publications::{Publication, Publications};
use crate::drivers::ble::mesh::config::subcriptions::Subscriptions;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyIndex, NetKeyIndex};
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use crate::drivers::ble::mesh::provisioning::IVUpdateFlag;
use crate::drivers::ble::mesh::{crypto, InsufficientBuffer};
use core::slice::Iter;
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Network {
    networks: Networks,
    iv_update_flag: IVUpdateFlag,
    iv_index: u32,
    unicast_address: UnicastAddress,
    subscriptions: Subscriptions,
}

impl Network {
    pub(crate) fn new(
        primary_network_details: NetworkDetails,
        iv_update_flag: IVUpdateFlag,
        iv_index: u32,
        unicast_address: UnicastAddress,
    ) -> Self {
        Self {
            networks: Networks::new(primary_network_details),
            iv_update_flag,
            iv_index,
            unicast_address,
            subscriptions: Default::default(),
        }
    }

    pub fn subscriptions(&self) -> &Subscriptions {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut Subscriptions {
        &mut self.subscriptions
    }

    pub(crate) fn find_by_net_key_index(
        &self,
        net_key_index: &NetKeyIndex,
    ) -> Result<&NetworkDetails, Status> {
        self.networks.find_by_index(net_key_index)
    }

    pub(crate) fn find_by_net_key_index_mut(
        &mut self,
        net_key_index: &NetKeyIndex,
    ) -> Result<&mut NetworkDetails, Status> {
        self.networks.find_by_index_mut(net_key_index)
    }

    pub(crate) fn find_by_app_key_index(
        &self,
        app_key_index: &AppKeyIndex,
    ) -> Result<&NetworkDetails, Status> {
        self.networks.find_by_app_key_index(app_key_index)
    }

    pub(crate) fn find_by_app_key_index_mut(
        &mut self,
        app_key_index: &AppKeyIndex,
    ) -> Result<&mut NetworkDetails, Status> {
        self.networks.find_by_app_key_index_mut(app_key_index)
    }

    pub(crate) fn find_app_key_by_aid(
        &self,
        aid: &ApplicationKeyIdentifier,
    ) -> Option<&AppKeyDetails> {
        self.networks.find_app_key_by_aid(aid)
    }

    pub(crate) fn find_publication(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<(&NetworkDetails, &Publication)> {
        self.networks
            .find_publication(element_address, model_identifier)
    }

    pub(crate) fn find_by_nid(
        &self,
        nid: u8,
    ) -> Result<Vec<NetworkDetails, 10>, InsufficientBuffer> {
        self.networks.find_by_nid(nid)
    }

    pub(crate) fn iter(&self) -> Iter<'_, NetworkDetails> {
        self.networks.iter()
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self, composition: &Composition) {
        info!("Primary unicast address: {}", self.unicast_address);
        info!("IV index: {:x}", self.iv_index);

        self.networks.display_configuration();

        info!("Elements:");
        for (i, element) in composition.elements.iter().enumerate() {
            let element_address = self.unicast_address + i as u8;
            info!("  {}: Address={}", i, element_address);
            for model in &element.models {
                info!("    - {}", model);
                self.networks.display_bindings(&element_address, &model);
                self.networks.display_publications(&element_address, &model);
                self.subscriptions
                    .display_subscriptions(&element_address, &model);
            }
        }
    }

    pub fn iv_index(&self) -> u32 {
        self.iv_index
    }

    pub fn unicast_address(&self) -> &UnicastAddress {
        &self.unicast_address
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Networks {
    networks: Vec<NetworkDetails, 3>,
}

impl Networks {
    fn new(primary_network_details: NetworkDetails) -> Self {
        let mut networks = Vec::new();
        networks.push(primary_network_details).ok();
        Self { networks }
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self) {
        for network in &self.networks {
            network.display_configuration()
        }
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_publications(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        for network in &self.networks {
            network.display_publications(element_address, model_identifier);
        }
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_bindings(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        for network in &self.networks {
            network.display_bindings(element_address, model_identifier)
        }
    }

    pub(crate) fn find_app_key_by_aid(
        &self,
        aid: &ApplicationKeyIdentifier,
    ) -> Option<&AppKeyDetails> {
        for network in &self.networks {
            if let Some(app_key_details) = network.find_app_key_by_aid(aid) {
                return Some(app_key_details);
            }
        }

        None
    }

    pub(crate) fn find_publication(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<(&NetworkDetails, &Publication)> {
        for network in &self.networks {
            if let Some(publication) = network.find_publication(element_address, model_identifier) {
                return Some((network, publication));
            }
        }

        None
    }

    pub(crate) fn iter(&self) -> Iter<'_, NetworkDetails> {
        self.networks.iter()
    }

    pub(crate) fn find_by_nid(
        &self,
        nid: u8,
    ) -> Result<Vec<NetworkDetails, 10>, InsufficientBuffer> {
        let mut found = Vec::new();
        for network in &self.networks {
            if network.nid == nid {
                // todo: remove this clone
                found
                    .push(network.clone())
                    .map_err(|_| InsufficientBuffer)?
            }
        }
        Ok(found)
    }

    pub(crate) fn find_by_index(
        &self,
        net_key_index: &NetKeyIndex,
    ) -> Result<&NetworkDetails, Status> {
        self.networks
            .iter()
            .find(|e| e.key_index == *net_key_index)
            .ok_or(Status::InvalidNetKeyIndex)
    }

    pub(crate) fn find_by_index_mut(
        &mut self,
        net_key_index: &NetKeyIndex,
    ) -> Result<&mut NetworkDetails, Status> {
        self.networks
            .iter_mut()
            .find(|e| e.key_index == *net_key_index)
            .ok_or(Status::InvalidNetKeyIndex)
    }

    pub(crate) fn find_by_app_key_index(
        &self,
        app_key_index: &AppKeyIndex,
    ) -> Result<&NetworkDetails, Status> {
        self.networks
            .iter()
            .find(|e| {
                matches!(
                    e.app_keys.iter().find(|a| a.index == *app_key_index),
                    Some(_)
                )
            })
            .ok_or(Status::InvalidAppKeyIndex)
    }

    pub(crate) fn find_by_app_key_index_mut(
        &mut self,
        app_key_index: &AppKeyIndex,
    ) -> Result<&mut NetworkDetails, Status> {
        self.networks
            .iter_mut()
            .find(|e| {
                matches!(
                    e.app_keys.iter().find(|a| a.index == *app_key_index),
                    Some(_)
                )
            })
            .ok_or(Status::InvalidAppKeyIndex)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct NetworkKey([u8; 16]);

impl NetworkKey {
    pub fn new(material: [u8; 16]) -> Self {
        Self(material)
    }
}

impl From<[u8; 16]> for NetworkKey {
    fn from(val: [u8; 16]) -> Self {
        Self(val)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for NetworkKey {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NetworkDetails {
    network_key: NetworkKey,
    key_index: NetKeyIndex,
    pub(crate) nid: u8,
    pub(crate) encryption_key: [u8; 16],
    pub(crate) privacy_key: [u8; 16],
    app_keys: Vec<AppKeyDetails, 10>,
    bindings: Bindings,
    publications: Publications,
}

impl NetworkDetails {
    pub fn new(
        network_key: NetworkKey,
        key_index: NetKeyIndex,
        nid: u8,
        encryption_key: [u8; 16],
        privacy_key: [u8; 16],
    ) -> Self {
        Self {
            network_key,
            key_index,
            nid,
            encryption_key,
            privacy_key,
            app_keys: Default::default(),
            bindings: Default::default(),
            publications: Default::default(),
        }
    }

    pub fn matches_nid(&self, nid: u8) -> bool {
        self.nid == nid
    }

    pub(crate) fn find_publication(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<&Publication> {
        self.publications.find(element_address, model_identifier)
    }

    pub(crate) fn publications(&self) -> &Publications {
        &self.publications
    }

    pub(crate) fn publications_mut(&mut self) -> &mut Publications {
        &mut self.publications
    }

    pub(crate) fn find_app_key_by_aid(
        &self,
        aid: &ApplicationKeyIdentifier,
    ) -> Option<&AppKeyDetails> {
        self.app_keys.iter().find(|e| e.aid == *aid)
    }

    pub(crate) fn find_app_key_by_index(
        &self,
        app_key_index: &AppKeyIndex,
    ) -> Option<&AppKeyDetails> {
        self.app_keys.iter().find(|e| e.index == *app_key_index)
    }

    pub(crate) fn app_keys_iter(&self) -> Iter<'_, AppKeyDetails> {
        self.app_keys.iter()
    }

    pub(crate) fn bind(
        &mut self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
        app_key_index: &AppKeyIndex,
    ) -> Result<(), Status> {
        self.bindings
            .bind(element_address, model_identifier, app_key_index)
    }

    pub(crate) fn unbind(
        &mut self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Result<(), Status> {
        self.bindings.unbind(element_address, model_identifier)
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_bindings(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        self.bindings
            .display_bindings(element_address, model_identifier);
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_publications(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        self.publications
            .display_publications(element_address, model_identifier);
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self) {
        info!("Network Keys:");
        info!(
            "  {}: {} [nid={}]",
            self.key_index, self.network_key, self.nid
        );
        info!("Application Keys:");
        for app_key in &self.app_keys {
            app_key.display_configuration();
        }
    }

    pub(crate) fn add_app_key(
        &mut self,
        app_key_index: AppKeyIndex,
        app_key: [u8; 16],
    ) -> Result<(), Status> {
        if let Some(_) = self.app_keys.iter().find(|e| e.index == app_key_index) {
            Err(Status::KeyIndexAlreadyStored)
        } else {
            let aid = crypto::k4(&app_key).map_err(|_| Status::UnspecifiedError)?;
            self.app_keys
                .push(AppKeyDetails {
                    aid: aid.into(),
                    key: app_key.into(),
                    index: app_key_index,
                })
                .map_err(|_| Status::InsufficientResources)?;
            Ok(())
        }
    }

    fn app_key_indexes(&self) -> Vec<AppKeyIndex, 10> {
        self.app_keys.iter().map(|e| e.index).collect()
    }

    fn is_valid_app_key_index(&self, app_key_index: AppKeyIndex) -> bool {
        matches!(
            self.app_keys.iter().find(|e| e.index == app_key_index),
            Some(_)
        )
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NetworkKeyHandle {
    pub(crate) network_key: NetworkKey,
    pub(crate) key_index: NetKeyIndex,
    pub(crate) nid: u8,
    pub(crate) encryption_key: [u8; 16],
    pub(crate) privacy_key: [u8; 16],
}

impl From<NetworkDetails> for NetworkKeyHandle {
    fn from(key: NetworkDetails) -> Self {
        Self {
            network_key: key.network_key,
            key_index: key.key_index,
            nid: key.nid,
            encryption_key: key.encryption_key,
            privacy_key: key.privacy_key,
        }
    }
}

impl From<&NetworkDetails> for NetworkKeyHandle {
    fn from(key: &NetworkDetails) -> Self {
        Self {
            network_key: key.network_key,
            key_index: key.key_index,
            nid: key.nid,
            encryption_key: key.encryption_key,
            privacy_key: key.privacy_key,
        }
    }
}
