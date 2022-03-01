use crate::drivers::ble::mesh::address::{Address, LabelUuid, UnicastAddress, VirtualAddress};
use crate::drivers::ble::mesh::model::foundation::configuration::model_subscription::SubscriptionAddress;
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Subscriptions {
    subscriptions: Vec<Subscription, 20>,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            subscriptions: Default::default(),
        }
    }
}

impl Subscriptions {
    #[cfg(feature = "defmt")]
    pub(crate) fn display_subscriptions(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        let mut matching: Vec<&Subscription, 20> = Vec::new();
        for e in self.subscriptions.iter().filter(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        }) {
            matching.push(e).ok();
        }

        if !matching.is_empty() {
            info!("      Subscriptions:");
            for subscription in matching.iter() {
                info!("        {}", subscription.subscription_address,);
            }
        }
    }

    pub(crate) fn add(
        &mut self,
        element_address: UnicastAddress,
        subscription_address: SubscriptionAddress,
        model_identifier: ModelIdentifier,
    ) -> Result<(), Status> {
        if let Some(_existing) = self.subscriptions.iter().find(|e| {
            e.element_address == element_address
                && e.subscription_address == subscription_address
                && e.model_identifier == model_identifier
        }) {
            // no harm, no foul
            Ok(())
        } else {
            self.subscriptions
                .push(Subscription {
                    element_address,
                    subscription_address,
                    model_identifier,
                })
                .map_err(|_| Status::InsufficientResources)?;
            Ok(())
        }
    }

    pub(crate) fn has_subscription(
        &self,
        element_address: &UnicastAddress,
        subscription_address: &SubscriptionAddress,
        model_identifier: &ModelIdentifier,
    ) -> bool {
        matches!(
            self.subscriptions
                .iter()
                .find(|e| e.element_address == *element_address
                    && e.model_identifier == *model_identifier
                    && e.subscription_address == *subscription_address),
            Some(_)
        )
    }

    pub(crate) fn has_any_subscription(&self, dst: &Address) -> bool {
        match dst {
            Address::Unicast(addr) => {
                let addr = SubscriptionAddress::Unicast(*addr);
                self.subscriptions
                    .iter()
                    .find(|e| e.subscription_address == addr)
                    .is_some()
            }
            Address::LabelUuid(addr) => {
                let addr = SubscriptionAddress::Virtual(*addr);
                self.subscriptions
                    .iter()
                    .find(|e| e.subscription_address == addr)
                    .is_some()
            }
            _ => false,
        }
    }

    pub(crate) fn find_label_uuids_by_address(
        &self,
        addr: VirtualAddress,
    ) -> Result<Vec<LabelUuid, 10>, InsufficientBuffer> {
        info!("find_label_uuids_by_address {}", addr);
        let mut uuids = Vec::new();

        for subscription in &self.subscriptions {
            if let SubscriptionAddress::Virtual(label_uuid) = subscription.subscription_address {
                if label_uuid.virtual_address() == addr {
                    if !uuids.contains(&label_uuid) {
                        uuids.push(label_uuid).map_err(|_| InsufficientBuffer)?;
                    }
                }
            }
        }

        info!("--> {}", uuids);

        Ok(uuids)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Subscription {
    element_address: UnicastAddress,
    subscription_address: SubscriptionAddress,
    model_identifier: ModelIdentifier,
}
