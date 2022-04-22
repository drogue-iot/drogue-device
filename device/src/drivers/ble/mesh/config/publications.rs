use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::model::foundation::configuration::AppKeyIndex;
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use embassy::time::Duration;
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Publications {
    publications: Vec<Publication, 10>,
}

impl Publications {
    pub(crate) fn find(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<&Publication> {
        self.publications.iter().find(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        })
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_publications(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        let mut matching: Vec<&Publication, 20> = Vec::new();
        for e in self.publications.iter().filter(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        }) {
            matching.push(e).ok();
        }

        if !matching.is_empty() {
            info!("      Publications:");
            for publication in matching.iter() {
                info!(
                    "        {} [{}] ttl={}",
                    publication.publish_address, publication.app_key_index, publication.publish_ttl,
                );
            }
        }
    }

    pub(crate) fn set(
        &mut self,
        element_address: UnicastAddress,
        publish_address: Address,
        app_key_index: AppKeyIndex,
        credential_flag: bool,
        publish_ttl: Option<u8>,
        publish_period: u8,
        publish_retransmit_count: u8,
        publish_retransmit_interval_steps: u8,
        model_identifier: ModelIdentifier,
    ) -> Result<(), Status> {
        if let Some(publication) = self.publications.iter_mut().find(|e| {
            e.element_address == element_address && e.model_identifier == model_identifier
        }) {
            publication.publish_address = publish_address;
            publication.credential_flag = credential_flag;
            publication.publish_ttl = publish_ttl;
            publication.publish_period = publish_period;
            publication.publish_retransmit_count = publish_retransmit_count;
            publication.publish_retransmit_interval_steps = publish_retransmit_interval_steps;
        } else {
            let publication = Publication {
                element_address,
                publish_address,
                app_key_index,
                credential_flag,
                publish_ttl,
                publish_period,
                publish_retransmit_count,
                publish_retransmit_interval_steps,
                model_identifier,
            };
            self.publications
                .push(publication)
                .map_err(|_| Status::InsufficientResources)?
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Publication {
    pub(crate) element_address: UnicastAddress,
    pub(crate) publish_address: Address,
    pub(crate) app_key_index: AppKeyIndex,
    pub(crate) credential_flag: bool,
    pub(crate) publish_ttl: Option<u8>,
    pub(crate) publish_period: u8,
    pub(crate) publish_retransmit_count: u8,
    pub(crate) publish_retransmit_interval_steps: u8,
    pub(crate) model_identifier: ModelIdentifier,
}
